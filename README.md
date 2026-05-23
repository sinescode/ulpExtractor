<div align="center">
  <img src="assets/logo.svg" alt="ulpExtractor" width="120">

  <br>

  [![Version](https://img.shields.io/badge/version-0.4.3-4ECDC4?style=flat-square&labelColor=1a1a2e)](https://github.com/sinescode/ulpExtractor/releases)
  [![Rust](https://img.shields.io/badge/rust-1.70+-000000?style=flat-square&logo=rust&logoColor=white&labelColor=1a1a2e)](https://rustup.rs)
  [![License](https://img.shields.io/badge/license-MIT-FF6B6B?style=flat-square&labelColor=1a1a2e)](LICENSE)
  [![Stars](https://img.shields.io/github/stars/sinescode/ulpExtractor?style=flat-square&color=FFD43B&labelColor=1a1a2e)](https://github.com/sinescode/ulpExtractor/stargazers)

  <br>

  A high-performance credential extractor that parses massive `url:user:pass` files and extracts matching entries by domain — boundary-aware matching, multi-threaded I/O, zero-copy memory maps.
</div>

---

## Features

| Category | Detail |
|----------|--------|
| **Domain matching** | Boundary-aware — catches subdomains, URLs, and emails; rejects false positives |
| **Performance** | Memory-mapped I/O, rayon parallelism, SIMD-accelerated byte search |
| **I/O** | Single-file, multi-file batch (`-a`), recursive directory walk (`-r`) |
| **Output control** | Deduplication, append mode (`-A`), match limit (`-M`), quiet mode (`-q`) |
| **Formats** | Configurable divider — `:`, `\|`, `;`, or any single-character separator |
| **UX** | Styled CLI with live progress bar, interactive prompt mode, graceful Ctrl-C |
| **Platform** | Linux, macOS, Windows — pre-built binaries on every release |

## Quick Start

```bash
# Pre-built binary (recommended)
curl -LO https://github.com/sinescode/ulpExtractor/releases/latest/download/ulpExtractor-linux-x86_64.tar.gz
tar xzf ulpExtractor-linux-x86_64.tar.gz
./ulpExtractor

# Or build from source
git clone https://github.com/sinescode/ulpExtractor.git && cd ulpExtractor
cargo build --release
./target/release/ulpExtractor
```

## Usage

### Single File

```bash
ulpExtractor -d netflix.com -i combo.txt
ulpExtractor -d netflix.com -i combo.txt -o results.txt
ulpExtractor -d netflix.com -i huge_dump.txt -M 100       # limit to 100 matches
ulpExtractor -d netflix.com -i combo.txt -q               # no progress bar
```

### Batch Scan

```bash
ulpExtractor -d netflix.com -a                             # all .txt in current dir
ulpExtractor -d netflix.com -a -x txt,csv,json              # specific extensions
ulpExtractor -d netflix.com -a -r                           # recursive
ulpExtractor -d netflix.com -a --dir ./data -t 8            # custom dir, 8 threads
ulpExtractor -d netflix.com -i extra.txt -A                 # append to existing output
```

### Interactive

```bash
ulpExtractor
```

Guided prompts for domain, input, output, threads, divider — same styled output as CLI.

## Options

| Flag | Description | Default |
|------|-------------|:------:|
| `-d, --domain` | Domain to extract credentials for | *required* |
| `-i, --input` | Input file path | — |
| `-a, --all` | Scan all files matching extensions in a directory | — |
| `-r, --recursive` | Walk directories recursively (with `-a`) | — |
| `-x, --extensions` | File extensions to include (comma-separated) | `txt` |
| `--dir` | Target directory for `-a` | `.` |
| `-o, --output` | Output file path | `output.txt` |
| `-A, --append` | Append to output instead of overwriting | — |
| `-t, --threads` | Number of worker threads (capped at 64) | `4` |
| `-D, --divider` | Field separator character | `:` |
| `-M, --max-matches` | Stop after N matches | unlimited |
| `-q, --quiet` | Suppress the progress bar | — |

## Input Format

```
<url_or_domain><divider><user><divider><password>
```

The domain may appear as a bare host, subdomain, inside a URL path, or in an email:

```
netflix.com:john:secret123
www.netflix.com:user@mail.com:pass456
https://platform.deepseek.com/login:admin:pass789
user@example.com:somepass
```

Matching is **boundary-aware** — `deepseek.com` matches `platform.deepseek.com` but not `mydeepseek.com`. Output is `user<divider>password`, one per line, deduplicated.

## Build

Requires **Rust 1.70+** ([rustup](https://rustup.rs)).

```bash
cargo build --release
# → target/release/ulpExtractor
```

