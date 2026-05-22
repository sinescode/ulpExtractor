use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use memchr::memchr;
use memmap2::Mmap;
use rayon::prelude::*;

pub struct ExtractProgress {
    pub processed: AtomicUsize,
    pub matched: AtomicUsize,
    pub total: AtomicUsize,
}

impl ExtractProgress {
    pub fn new(total: usize) -> Self {
        Self { processed: AtomicUsize::new(0), matched: AtomicUsize::new(0), total: AtomicUsize::new(total) }
    }
}

pub struct ExtractResult {
    pub matched_count: usize,
    pub total_bytes: u64,
    pub duration_ms: u64,
}

/// Find all files in `dir` matching any of the given extensions.
pub fn find_files(dir: &Path, extensions: &[String]) -> std::io::Result<Vec<PathBuf>> {
    let exts: Vec<String> = extensions
        .iter()
        .map(|e| {
            let e = e.trim();
            if e.starts_with('.') { e.to_lowercase() } else { format!(".{}", e.to_lowercase()) }
        })
        .collect();

    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let matches = path.extension()
                .and_then(|e| e.to_str())
                .map(|e| exts.iter().any(|ext| ext[1..] == e.to_lowercase()))
                .unwrap_or(false);
            if matches { files.push(path); }
        }
    }
    files.sort();
    Ok(files)
}

/// Sum file sizes (instant — just metadata).
pub fn total_bytes(paths: &[PathBuf]) -> std::io::Result<u64> {
    let mut total = 0u64;
    for p in paths {
        if let Ok(meta) = fs::metadata(p) {
            total += meta.len();
        }
    }
    Ok(total)
}

/// Extract from a single file. Delegates to extract_multi.
pub fn extract(
    input_path: &Path,
    domain: &str,
    divider: char,
    threads: usize,
    output_path: &Path,
    progress: Arc<ExtractProgress>,
    cancelled: Arc<AtomicBool>,
) -> std::io::Result<ExtractResult> {
    extract_multi(&[input_path.to_path_buf()], domain, divider, threads, output_path, progress, cancelled)
}

/// Extract from multiple files into a single output.
///
/// Uses memory-mapped I/O (zero-copy), rayon work-stealing parallelism,
/// and SIMD-accelerated byte search via memchr.
pub fn extract_multi(
    input_paths: &[PathBuf],
    domain: &str,
    divider: char,
    threads: usize,
    output_path: &Path,
    progress: Arc<ExtractProgress>,
    cancelled: Arc<AtomicBool>,
) -> std::io::Result<ExtractResult> {
    let start = Instant::now();
    let domain = domain.as_bytes();
    let div = divider as u8;
    let threads = threads.max(1);

    // Pre-scan total bytes
    let total = total_bytes(input_paths)?;
    progress.total.store(total as usize, Ordering::Relaxed);

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut out = File::create(output_path)?;
    let mut total_matched = 0usize;
    let mut bytes_done = 0usize;

    // Build rayon thread pool once
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();

    for input_path in input_paths {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }

        let file = File::open(input_path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let data: &[u8] = &mmap;

        if data.is_empty() {
            continue;
        }

        // ── Split into aligned chunks ──────────────────────────────────
        let num_chunks = (threads * 4).min(data.len() / 4096).max(1);
        let approx = data.len() / num_chunks;

        let mut chunks: Vec<(usize, &[u8])> = Vec::with_capacity(num_chunks);
        let mut offset = 0;

        for i in 0..num_chunks {
            if offset >= data.len() { break; }
            let end = if i == num_chunks - 1 {
                data.len()
            } else {
                let target = (offset + approx).min(data.len());
                match memchr(b'\n', &data[target..]) {
                    Some(pos) => target + pos + 1,
                    None => data.len(),
                }
            };
            chunks.push((offset, &data[offset..end]));
            offset = end;
        }

        // ── Parallel extraction ────────────────────────────────────────
        let results: Vec<Vec<u8>> = pool.install(|| {
            chunks
                .par_iter()
                .flat_map(|&(_, chunk)| {
                    let mut local = Vec::new();
                    let mut pos = 0;

                    while pos < chunk.len() {
                        if cancelled.load(Ordering::Relaxed) {
                            break;
                        }

                        let line_end = memchr(b'\n', &chunk[pos..])
                            .map(|p| pos + p)
                            .unwrap_or(chunk.len());

                        let mut line = &chunk[pos..line_end];
                        if line.last() == Some(&b'\r') {
                            line = &line[..line.len() - 1];
                        }

                        if let Some(div_pos) = memchr(div, line) {
                            if &line[..div_pos] == domain {
                                local.push(line[div_pos + 1..].to_vec());
                            }
                        }

                        pos = line_end + 1;
                    }

                    local
                })
                .collect()
        });

        bytes_done += data.len();
        progress.processed.store(bytes_done, Ordering::Relaxed);
        total_matched += results.len();

        for line in &results {
            out.write_all(line)?;
            out.write_all(b"\n")?;
        }
    }

    progress.matched.store(total_matched, Ordering::Relaxed);
    progress.processed.store(total as usize, Ordering::Relaxed);

    Ok(ExtractResult {
        matched_count: total_matched,
        total_bytes: total,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}
