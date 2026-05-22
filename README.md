# ulpExtractor

**Fast domain credential extractor** — parse large `domain:user:pass` lists and extract matching credentials by domain.

Built in Rust with a dual-mode interface: interactive TUI for daily use, CLI for scripting and automation.

## Features

- **Interactive TUI** — form-based input, file browser, live progress bar, results summary
- **CLI mode** — pipe-friendly, ideal for shell scripts and automation
- **Multi-threaded** — configurable parallel extraction, saturates I/O on any file size
- **Configurable divider** — works with `:`, `|`, `;`, or any single-character separator
- **Cross-platform** — Linux, macOS, Windows pre-built binaries on every release

## Quick Start

```bash
# Download the latest binary for your platform from Releases, or build from source:
git clone https://github.com/sinescode/ulpExtractor.git
cd ulpExtractor
cargo build --release
./target/release/ulpExtractor
```

## Usage

### TUI Mode

```bash
./ulpExtractor
```

Launches an interactive terminal interface where you can:
1. Enter the domain to filter by
2. Set thread count and field divider
3. Browse and select input/output files
4. Watch live extraction progress
5. Review results

### CLI Mode

```bash
ulpExtractor \
  -d "fiverr.com" \
  -t 8 \
  -D ":" \
  -i combo.txt \
  -o extracted.txt
```

### Options

| Flag | Description | Default |
|------|-------------|---------|
| `-d, --domain` | Domain to match (first field) | required |
| `-t, --threads` | Number of threads | `4` |
| `-D, --divider` | Field separator character | `:` |
| `-i, --input` | Input file path | required |
| `-o, --output` | Output file path | required |

## Input Format

Each line should be `domain<divider>user<divider>password`:

```
fiverr.com:estheticdesigns:Ahmadraza
google.com:user@gmail.com:password123
```

Output is `user<divider>password` for matching lines only.

## Build from Source

Requires Rust **1.70+** (install via [rustup](https://rustup.rs)).

```bash
cargo build --release
```

Binary lands at `target/release/ulpExtractor`.
