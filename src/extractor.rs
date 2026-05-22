use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
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
    #[allow(dead_code)]
    pub total_lines: usize,
    pub duration_ms: u64,
}

pub fn count_lines(path: &Path) -> std::io::Result<usize> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let count = reader.lines().count();
    Ok(count)
}

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

    let results = Arc::new(Mutex::new(Vec::new()));
    let threads = threads.min(total).max(1);
    let chunk_size = (total + threads - 1) / threads;

    thread::scope(|s| {
        for chunk in lines.chunks(chunk_size) {
            let results = Arc::clone(&results);
            let progress = Arc::clone(&progress);
            let cancelled = Arc::clone(&cancelled);
            let domain = domain.to_string();

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
                // Flush remainder
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
