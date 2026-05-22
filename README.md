# ulpExtractor

**Fast domain credential extractor** — parse large `domain:user:pass` lists and extract matching credentials by domain.

Built in Rust with a styled CLI, interactive prompt mode, and multi-file batch scanning.

## Features

- **Styled CLI** — boxed header, colored fields, live progress bar with real-time match counter
- **Interactive mode** — guided prompts when run with no arguments, same visual design as CLI
- **Multi-file scan** (`-a`) — scan all files in a directory matching given extensions
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

### Interactive Mode

```bash
./ulpExtractor
```

Prompts you for each field with defaults — same styled output as CLI mode.

### CLI — Single File

```bash
ulpExtractor -d netflix.com -i combo.txt -o extracted.txt
```

### CLI — Multi-File Scan

```bash
# Scan all .txt files in current directory
ulpExtractor -d netflix.com -a

# Scan specific extensions
ulpExtractor -d netflix.com -a -x txt,json,csv

# Scan a different directory
ulpExtractor -d netflix.com -a --dir ./data -o results.txt -t 8
```

### Options

| Flag | Description | Default |
|------|-------------|---------|
| `-d, --domain` | Domain to match (first field) | required |
| `-i, --input` | Input file path (single-file mode) | — |
| `-a, --all` | Scan all files matching extensions | off |
| `-x, --extensions` | File extensions to scan, comma-separated | `txt` |
| `--dir` | Directory to scan when using `-a` | `.` |
| `-o, --output` | Output file path | `output.txt` |
| `-t, --threads` | Number of threads | `4` |
| `-D, --divider` | Field separator character | `:` |

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
