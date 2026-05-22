mod extractor;

use clap::Parser;
use console::{style, Term};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

// ── CLI Args ─────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "ulpExtractor",
    about = "Extract user:pass pairs for a given domain from credential files",
    after_help = "\
Examples:
  ulpExtractor -d netflix.com -i dump.txt
  ulpExtractor -d netflix.com -a
  ulpExtractor -d netflix.com -a -x txt,json,csv
  ulpExtractor -d netflix.com -a --dir ./data -o results.txt -t 8
  ulpExtractor"
)]
struct Args {
    #[arg(short, long)]
    domain: Option<String>,
    #[arg(short, long)]
    input: Option<PathBuf>,
    #[arg(short, long, default_value = "output.txt")]
    output: PathBuf,
    #[arg(short, long, default_value = "4")]
    threads: usize,
    #[arg(short = 'D', long, default_value = ":")]
    divider: char,
    /// Scan all files matching extensions in a directory
    #[arg(short = 'a', long)]
    all: bool,
    /// File extensions to scan (comma-separated). Default: txt
    #[arg(short = 'x', long = "extensions", default_value = "txt", value_delimiter = ',')]
    extensions: Vec<String>,
    /// Directory to scan when using --all (defaults to current dir)
    #[arg(long = "dir", default_value = ".")]
    dir: PathBuf,
    /// Scan directories recursively (only with --all)
    #[arg(short = 'r', long)]
    recursive: bool,
    /// Append to output file instead of overwriting
    #[arg(short = 'A', long)]
    append: bool,
}

// ── Entry Point ──────────────────────────────────────────────────────────

fn main() -> std::io::Result<()> {
    if std::env::args().len() == 1 {
        interactive_mode()
    } else {
        cli_mode(Args::parse())
    }
}

// ── Shared UI Helpers ────────────────────────────────────────────────────

fn print_header() {
    let term = Term::stdout();
    let w = term.size().1 as usize;

    let top = "┌".to_string() + &"─".repeat(w.saturating_sub(2)) + "┐";
    let bot = "└".to_string() + &"─".repeat(w.saturating_sub(2)) + "┘";

    let title = " ulpExtractor v0.4.2 ";
    let subtitle = " Domain credential extractor ";

    let pad = (w.saturating_sub(title.len())) / 2;
    let title_line = "│".to_string()
        + &" ".repeat(pad) + title
        + &" ".repeat(w.saturating_sub(pad + title.len() + 1))
        + "│";

    let pad_s = (w.saturating_sub(subtitle.len())) / 2;
    let sub_line = "│".to_string()
        + &" ".repeat(pad_s) + subtitle
        + &" ".repeat(w.saturating_sub(pad_s + subtitle.len() + 1))
        + "│";

    println!("{}", style(top).cyan());
    println!("{}", style(title_line).cyan().bold());
    println!("{}", style(sub_line).cyan().dim());
    println!("{}", style(bot).cyan());
    println!();
}

fn print_field(label: &str, value: &str) {
    println!("  {} {}", style(format!("{}:", label)).cyan().bold(), style(value).white());
}

fn print_divider() {
    let term = Term::stdout();
    let w = term.size().1 as usize;
    println!("  {}", style("─".repeat(w.saturating_sub(4))).dim());
}

fn new_progress_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {spinner:.cyan} {msg:36} [{bar:30.cyan/blue}] {percent:>3}%  {bytes:.dim}/{total_bytes:.dim}")
            .unwrap()
            .progress_chars("━╸─"),
    );
    pb.set_message("Extracting...".to_string());
    pb
}

// ── CLI Mode ─────────────────────────────────────────────────────────────

fn cli_mode(args: Args) -> std::io::Result<()> {
    print_header();

    let domain = match &args.domain {
        Some(d) => d.trim().to_string(),
        None => {
            eprintln!("{}", style("Error: --domain is required").red().bold());
            std::process::exit(1);
        }
    };

    let threads = args.threads.max(1).min(64);
    let input_files: Vec<PathBuf> = resolve_input_files(&args)?;

    // Print config
    let exts_display = args.extensions.iter()
        .map(|e| if e.starts_with('.') { e.clone() } else { format!(".{}", e) })
        .collect::<Vec<_>>()
        .join(", ");

    print_field("Domain", &domain);
    if args.all {
        let mode_label = if args.recursive { "all recursive" } else { "all" };
        print_field("Mode", &format!("{} ({}) — {} files", mode_label, exts_display, input_files.len()));
        for f in &input_files {
            println!("         {}", style(f.file_name().unwrap_or_default().to_string_lossy()).dim());
        }
    } else {
        print_field("Input", &input_files[0].display().to_string());
    }
    print_field("Output", &format!("{} {}", args.output.display().to_string(), if args.append { "(append)" } else { "" }));
    print_field("Threads", &threads.to_string());
    print_field("Divider", &format!("'{}'", args.divider));
    print_divider();

    // Total size from metadata (instant — no file reading)
    let total_bytes = extractor::total_bytes(&input_files)?;
    println!(
        "  {} {}",
        style("Total size:").cyan(),
        style(format_bytes(total_bytes)).white().bold()
    );
    println!();

    run_extraction(&input_files, &domain, args.divider, threads, &args.output, total_bytes, args.append)
}

// ── Interactive Mode ─────────────────────────────────────────────────────

fn interactive_mode() -> std::io::Result<()> {
    print_header();
    println!("  {}  Fill in the fields below (press Enter to skip optional)\n", style("?").yellow());

    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());

    // Domain
    let domain = loop {
        print!("  {} ", style("Domain:").cyan().bold());
        std::io::stdout().flush()?;
        let mut s = String::new();
        reader.read_line(&mut s)?;
        let s = s.trim().to_string();
        if !s.is_empty() { break s; }
        println!("  {}", style("  Error: Domain is required").red());
    };

    // Mode
    print!("  {} [file/all] (default: file): ", style("Mode:").cyan().bold());
    std::io::stdout().flush()?;
    let mut mode = String::new();
    reader.read_line(&mut mode)?;
    let is_all = matches!(mode.trim().to_lowercase().as_str(), "all" | "a");

    let input_files: Vec<PathBuf> = if is_all {
        print!("  {} (default: .): ", style("Directory:").cyan().bold());
        std::io::stdout().flush()?;
        let mut dir_s = String::new();
        reader.read_line(&mut dir_s)?;
        let dir = if dir_s.trim().is_empty() { PathBuf::from(".") } else { PathBuf::from(dir_s.trim()) };

        print!("  {} (y/N): ", style("Recursive:").cyan().bold());
        std::io::stdout().flush()?;
        let mut rec_s = String::new();
        reader.read_line(&mut rec_s)?;
        let recursive = matches!(rec_s.trim().to_lowercase().as_str(), "y" | "yes");

        print!("  {} (comma-separated, default: txt): ", style("Extensions:").cyan().bold());
        std::io::stdout().flush()?;
        let mut exts = String::new();
        reader.read_line(&mut exts)?;
        let exts: Vec<String> = if exts.trim().is_empty() {
            vec!["txt".to_string()]
        } else {
            exts.trim().split(',').map(|s| s.trim().to_string()).collect()
        };

        let found = extractor::find_files(&dir, &exts, recursive)?;
        if found.is_empty() {
            eprintln!("{}", style(format!("No files in '{}' with extensions: {:?}", dir.display(), exts)).red());
            std::process::exit(1);
        }
        found
    } else {
        print!("  {} ", style("Input file:").cyan().bold());
        std::io::stdout().flush()?;
        let mut input_s = String::new();
        reader.read_line(&mut input_s)?;
        let input_s = input_s.trim().to_string();
        if input_s.is_empty() {
            eprintln!("{}", style("Error: Input file is required").red().bold());
            std::process::exit(1);
        }
        let p = PathBuf::from(&input_s);
        if !p.exists() {
            eprintln!("{}", style(format!("Error: file not found: {}", p.display())).red().bold());
            std::process::exit(1);
        }
        vec![p]
    };

    // Output
    print!("  {} (default: output.txt): ", style("Output file:").cyan().bold());
    std::io::stdout().flush()?;
    let mut output_s = String::new();
    reader.read_line(&mut output_s)?;
    let output = if output_s.trim().is_empty() { PathBuf::from("output.txt") } else { PathBuf::from(output_s.trim()) };

    // Append
    print!("  {} (y/N): ", style("Append to output:").cyan().bold());
    std::io::stdout().flush()?;
    let mut append_s = String::new();
    reader.read_line(&mut append_s)?;
    let append = matches!(append_s.trim().to_lowercase().as_str(), "y" | "yes");

    // Threads
    print!("  {} (default: 4): ", style("Threads:").cyan().bold());
    std::io::stdout().flush()?;
    let mut threads_s = String::new();
    reader.read_line(&mut threads_s)?;
    let threads: usize = threads_s.trim().parse().unwrap_or(4).max(1).min(64);

    // Divider
    print!("  {} (default: :): ", style("Divider:").cyan().bold());
    std::io::stdout().flush()?;
    let mut divider_s = String::new();
    reader.read_line(&mut divider_s)?;
    let divider = if divider_s.trim().is_empty() { ':' } else { divider_s.trim().chars().next().unwrap_or(':') };

    println!();
    print_divider();

    let total_bytes = extractor::total_bytes(&input_files)?;
    println!("  {} {}", style("Total size:").cyan(), style(format_bytes(total_bytes)).white().bold());
    println!();

    run_extraction(&input_files, &domain, divider, threads, &output, total_bytes, append)
}

// ── Shared Extraction Runner ─────────────────────────────────────────────

fn run_extraction(
    input_files: &[PathBuf],
    domain: &str,
    divider: char,
    threads: usize,
    output_path: &PathBuf,
    total_bytes: u64,
    append: bool,
) -> std::io::Result<()> {
    let total = total_bytes as usize;

    let pb = new_progress_bar(total as u64);
    let cancelled = Arc::new(AtomicBool::new(false));

    // Ctrl-C handler — set cancelled flag so extraction flushes partial results
    let c_clone = Arc::clone(&cancelled);
    ctrlc::set_handler(move || {
        c_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    let progress = Arc::new(extractor::ExtractProgress::new(total));
    let p_clone = Arc::clone(&progress);
    let pb_clone = pb.clone();
    let canc_clone = Arc::clone(&cancelled);

    // Background thread: poll progress and update bar
    std::thread::spawn(move || loop {
        let processed = p_clone.processed.load(std::sync::atomic::Ordering::Relaxed);
        let matched = p_clone.matched.load(std::sync::atomic::Ordering::Relaxed);
        let tot = p_clone.total.load(std::sync::atomic::Ordering::Relaxed);

        pb_clone.set_position(processed as u64);
        pb_clone.set_message(format!("Matches: {}", format_number(matched as u64)));

        let stopped = canc_clone.load(std::sync::atomic::Ordering::Relaxed);
        if processed >= tot && tot > 0 || stopped {
            if stopped {
                pb_clone.finish_with_message(format!("Cancelled — {} matches flushed", format_number(matched as u64)));
            } else {
                pb_clone.finish_with_message(format!("Done — {} matches", format_number(matched as u64)));
            }
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    });

    let result = if input_files.len() == 1 {
        extractor::extract(
            input_files[0].as_path(),
            domain,
            divider,
            threads,
            output_path,
            progress,
            cancelled.clone(),
            append,
        )?
    } else {
        extractor::extract_multi(
            input_files,
            domain,
            divider,
            threads,
            output_path,
            progress,
            cancelled.clone(),
            append,
        )?
    };

    pb.finish_and_clear();

    // Summary
    let was_cancelled = cancelled.load(std::sync::atomic::Ordering::Relaxed);
    println!();
    if was_cancelled {
        println!(
            "  {}  {} matches flushed from {} across {} file(s) in {:.1}s (cancelled)",
            style("!").yellow().bold(),
            style(format_number(result.matched_count as u64)).yellow().bold(),
            style(format_bytes(result.total_bytes)).white(),
            input_files.len(),
            result.duration_ms as f64 / 1000.0
        );
    } else {
        println!(
            "  {}  {} matches from {} across {} file(s) in {:.1}s",
            style("✓").green().bold(),
            style(format_number(result.matched_count as u64)).green().bold(),
            style(format_bytes(result.total_bytes)).white(),
            input_files.len(),
            result.duration_ms as f64 / 1000.0
        );
    }
    println!(
        "  {}  {}",
        style("→").dim(),
        style(output_path.display().to_string()).dim()
    );
    println!();

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────

fn resolve_input_files(args: &Args) -> Result<Vec<PathBuf>, std::io::Error> {
    if args.all {
        let found = extractor::find_files(&args.dir, &args.extensions, args.recursive)?;
        if found.is_empty() {
            eprintln!("{}", style(format!(
                "No files found in '{}' with extensions: {:?}",
                args.dir.display(), args.extensions
            )).red());
            std::process::exit(1);
        }
        Ok(found)
    } else {
        match &args.input {
            Some(p) => {
                if !p.exists() {
                    eprintln!("{}", style(format!("Error: file not found: {}", p.display())).red().bold());
                    std::process::exit(1);
                }
                Ok(vec![p.clone()])
            }
            None => {
                eprintln!("{}", style("Error: --input or --all is required").red().bold());
                std::process::exit(1);
            }
        }
    }
}

fn format_bytes(n: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = n as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} B", n)
    } else {
        format!("{:.1} {}", size, UNITS[unit])
    }
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

