use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

pub struct ExtractProgress {
    pub processed: AtomicUsize,
    pub matched: AtomicUsize,
    pub total: AtomicUsize,
}

impl ExtractProgress {
    pub fn new(total: usize) -> Self {
        Self {
            processed: AtomicUsize::new(0),
            matched: AtomicUsize::new(0),
            total: AtomicUsize::new(total),
        }
    }
}

pub struct ExtractResult {
    pub matched_count: usize,
    pub total_lines: usize,
    pub duration_ms: u64,
}

/// Find all files in `dir` matching any of the given extensions.
/// Extensions are normalized to start with a dot (e.g. "txt" → ".txt").
pub fn find_files(dir: &Path, extensions: &[String]) -> std::io::Result<Vec<PathBuf>> {
    let exts: Vec<String> = extensions
        .iter()
        .map(|e| {
            let e = e.trim();
            if e.starts_with('.') {
                e.to_lowercase()
            } else {
                format!(".{}", e.to_lowercase())
            }
        })
        .collect();

    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let matches = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| exts.iter().any(|ext| ext[1..] == e.to_lowercase()))
                .unwrap_or(false);
            if matches {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub fn count_lines(path: &Path) -> std::io::Result<usize> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    Ok(reader.lines().count())
}

pub fn count_lines_multi(paths: &[PathBuf]) -> std::io::Result<usize> {
    let mut total = 0;
    for p in paths {
        total += count_lines(p)?;
    }
    Ok(total)
}

/// Extract from a single file. Used for both single-file mode and as a building block.
pub fn extract(
    input_path: &Path,
    domain: &str,
    divider: char,
    threads: usize,
    output_path: &Path,
    progress: Arc<ExtractProgress>,
    cancelled: Arc<AtomicBool>,
) -> std::io::Result<ExtractResult> {
    let start = Instant::now();

    let file = File::open(input_path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

    let total = lines.len();
    progress.total.store(total, Ordering::Relaxed);

    if total == 0 {
        return Ok(ExtractResult {
            matched_count: 0,
            total_lines: 0,
            duration_ms: start.elapsed().as_millis() as u64,
        });
    }

    let results = Arc::new(Mutex::new(Vec::new()));
    let threads = threads.min(total).max(1);
    let chunk_size = (total + threads - 1) / threads;

    thread::scope(|s| {
        for chunk in lines.chunks(chunk_size) {
            let results = Arc::clone(&results);
            let progress = Arc::clone(&progress);
            let cancelled = Arc::clone(&cancelled);

            s.spawn(move || {
                let mut local = Vec::new();
                let mut local_processed = 0usize;

                for line in chunk {
                    if cancelled.load(Ordering::Relaxed) {
                        return;
                    }

                    let parts: Vec<&str> = line.splitn(3, divider).collect();
                    if parts.len() == 3 && parts[0] == domain {
                        local.push(format!("{}{}{}", parts[1], divider, parts[2]));
                    }

                    local_processed += 1;
                    if local_processed % 512 == 0 {
                        progress.processed.fetch_add(512, Ordering::Relaxed);
                        local_processed -= 512;
                    }
                }
                if local_processed > 0 {
                    progress.processed.fetch_add(local_processed, Ordering::Relaxed);
                }
                progress.matched.fetch_add(local.len(), Ordering::Relaxed);

                if !cancelled.load(Ordering::Relaxed) {
                    results.lock().unwrap().extend(local);
                }
            });
        }
    });

    if cancelled.load(Ordering::Relaxed) {
        return Ok(ExtractResult {
            matched_count: 0,
            total_lines: total,
            duration_ms: start.elapsed().as_millis() as u64,
        });
    }

    let results = results.lock().unwrap();
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut out = File::create(output_path)?;
    for line in results.iter() {
        writeln!(out, "{}", line)?;
    }

    Ok(ExtractResult {
        matched_count: results.len(),
        total_lines: total,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

/// Extract from multiple files into a single output.
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
    let mut total_matched = 0usize;
    let mut total_lines = 0usize;

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut out = File::create(output_path)?;

    for input_path in input_paths {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }

        let file = File::open(input_path)?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

        let file_total = lines.len();
        if file_total == 0 {
            continue;
        }
        total_lines += file_total;

        let results = Arc::new(Mutex::new(Vec::new()));
        let t = threads.min(file_total).max(1);
        let chunk_size = (file_total + t - 1) / t;

        thread::scope(|s| {
            for chunk in lines.chunks(chunk_size) {
                let results = Arc::clone(&results);
                let progress = Arc::clone(&progress);
                let cancelled = Arc::clone(&cancelled);

                s.spawn(move || {
                    let mut local = Vec::new();
                    let mut local_processed = 0usize;

                    for line in chunk {
                        if cancelled.load(Ordering::Relaxed) {
                            return;
                        }

                        let parts: Vec<&str> = line.splitn(3, divider).collect();
                        if parts.len() == 3 && parts[0] == domain {
                            local.push(format!("{}{}{}", parts[1], divider, parts[2]));
                        }

                        local_processed += 1;
                        if local_processed % 512 == 0 {
                            progress.processed.fetch_add(512, Ordering::Relaxed);
                            local_processed -= 512;
                        }
                    }
                    if local_processed > 0 {
                        progress.processed.fetch_add(local_processed, Ordering::Relaxed);
                    }
                    progress.matched.fetch_add(local.len(), Ordering::Relaxed);

                    if !cancelled.load(Ordering::Relaxed) {
                        results.lock().unwrap().extend(local);
                    }
                });
            }
        });

        if cancelled.load(Ordering::Relaxed) {
            break;
        }

        let results = results.lock().unwrap();
        total_matched += results.len();
        for line in results.iter() {
            writeln!(out, "{}", line)?;
        }
    }

    Ok(ExtractResult {
        matched_count: total_matched,
        total_lines,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}
