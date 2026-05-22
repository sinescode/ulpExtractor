mod extractor;
mod tui;

use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "ulpExtractor", about = "Extract user:pass pairs for a given domain")]
struct Args {
    /// Domain to filter by (first field in each line)
    #[arg(short, long)]
    domain: String,

    /// Number of threads for parallel processing
    #[arg(short, long, default_value = "4")]
    threads: usize,

    /// Character that separates domain, user, and password
    #[arg(short = 'D', long, default_value = ":")]
    divider: char,

    /// Input file path
    #[arg(short, long)]
    input: PathBuf,

    /// Output file path
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> std::io::Result<()> {
    if std::env::args().len() == 1 {
        // No arguments -> TUI mode
        tui::run()
    } else {
        // Arguments provided -> CLI mode
        cli_run(Args::parse())
    }
}

fn cli_run(args: Args) -> std::io::Result<()> {
    let total = extractor::count_lines(&args.input)?;

    println!("Domain:    {}", args.domain);
    println!("Input:     {}", args.input.display());
    println!("Output:    {}", args.output.display());
    println!("Threads:   {}", args.threads);
    println!("Divider:   '{}'", args.divider);
    println!("Lines:     {}", total);
    println!();

    let progress = Arc::new(extractor::ExtractProgress::new(total));
    let cancelled = Arc::new(AtomicBool::new(false));

    let result = extractor::extract(
        &args.input,
        &args.domain,
        args.divider,
        args.threads,
        &args.output,
        progress,
        cancelled,
    )?;

    println!(
        "Done - {} matches extracted in {:.1}s",
        result.matched_count,
        result.duration_ms as f64 / 1000.0
    );
    Ok(())
}
