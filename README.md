# ulpExtractor

**Fast domain credential extractor** — parse large `url:user:pass` lists and extract matching credentials by domain.

Built in Rust with a styled CLI, interactive prompt mode, and multi-file batch scanning.

## Features

- **Smart domain matching** — boundary-aware, matches subdomains (`www.netflix.com` matches `netflix.com`), URLs (`https://deepseek.com/path:user:pass`), and emails (`user@domain.com`); rejects false positives like `mydeepseek.com`
- **Styled CLI** — boxed header, colored fields, live progress bar with real-time match counter
- **Interactive mode** — guided prompts when run with no arguments, same visual design as CLI
- **Multi-file scan** (`-a`) — scan all files in a directory matching given extensions
- **Recursive scan** (`-r`) — recursive directory walk when combined with `-a`
- **Multi-threaded** — configurable parallel extraction, saturates I/O on any file size
- **Configurable divider** — works with `:`, `|`, `;`, or any single-character separator
- **Append mode** (`-A`) — append to output file instead of overwriting
- **Deduplication** — duplicate `user:pass` pairs are written only once
- **Graceful cancel** — Ctrl-C flushes partial results to output before exiting
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

# Scan recursively
ulpExtractor -d netflix.com -a -r

# Scan a different directory
ulpExtractor -d netflix.com -a --dir ./data -o results.txt -t 8

# Append to existing output
ulpExtractor -d netflix.com -i combo2.txt -A
```

### Options

| Flag | Description | Default |
|------|-------------|---------|
| `-d, --domain` | Domain to match (first field) | required |
| `-i, --input` | Input file path (single-file mode) | — |
| `-a, --all` | Scan all files matching extensions | off |
| `-r, --recursive` | Scan directories recursively (with `-a`) | off |
| `-x, --extensions` | File extensions to scan, comma-separated | `txt` |
| `--dir` | Directory to scan when using `-a` | `.` |
| `-o, --output` | Output file path | `output.txt` |
| `-A, --append` | Append to output instead of overwriting | off |
| `-t, --threads` | Number of threads (capped at 64) | `4` |
| `-D, --divider` | Field separator character | `:` |

## Input Format

Lines use `<url_or_domain><divider><user><divider><password>`. The domain can appear anywhere in the URL portion — bare, as a subdomain, inside an `https://` URL with paths, or in an email:

```
netflix.com:john:secret123
www.netflix.com:user@mail.com:pass456
https://platform.deepseek.com/login:admin:pass789
user@example.com:somepass
```

Matching is **boundary-aware** — `deepseek.com` matches `platform.deepseek.com` but NOT `mydeepseek.com`.

Output is `user<divider>password` for matching lines only. Lines without a user portion (`domain:pass`) are skipped.

## Upgrade Notes

### v0.4.0 → v0.4.1
- **Graceful cancel**: Ctrl-C now flushes partial results before exiting
- **Append mode** (`-A`): append to output instead of overwriting
- **Recursive scan** (`-r`): scan subdirectories with `-a`
- **Deduplication**: duplicate `user:pass` pairs written only once
- **Thread cap**: threads capped at 64 (was unbounded)
- **Code cleanup**: removed duplicate formatting functions

### v0.3.x → v0.4.0
v0.4.0 introduces **smart domain matching**:
- **Subdomain matching**: `netflix.com` now matches `www.netflix.com`, `login.netflix.com`, etc.
- **URL support**: URLs like `https://domain.com/path:user:pass` are parsed correctly
- **Boundary detection**: `deepseek.com` no longer false-matches `mydeepseek.com`
- **Output format**: always `user:pass` — lines without a user portion are skipped

## Build from Source

Requires Rust **1.70+** (install via [rustup](https://rustup.rs)).

```bash
cargo build --release
```

Binary lands at `target/release/ulpExtractor`.
