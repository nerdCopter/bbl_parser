# Crate Usage: bbl_parser

Focused guidance for using the bbl_parser Rust crate.

## Table of Contents
- [Installation](#installation)
- [Cargo features](#cargo-features)
- [Basic usage](#basic-usage)
- [Multi-log processing](#multi-log-processing)
- [Parsing from memory](#parsing-from-memory)
- [Examples](#examples)
- [Notes](#notes)

## Installation

Add the dependency to your Cargo.toml:

```toml
[dependencies]
# Local path while developing; use a version when published
bbl_parser = { path = "path/to/bbl_parser" }
```

## Cargo features

- `csv` (default): CSV export helpers
- `cli` (default): Command-line entry points
- `json`: JSON-related helpers (requires `serde`)
- `serde`: Enable serialization for data structures

If you only need the parser types and functions, the defaults are fine.

## Basic usage

```rust
use bbl_parser::{parse_bbl_file, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let log = parse_bbl_file(Path::new("flight.BBL"), ExportOptions::default(), false)?;
    println!("firmware: {}", log.header.firmware_revision);
    println!("frames: {}", log.sample_frames.len());
    Ok(())
}
```

Key outputs on the BBLLog:
- `header`: configuration and metadata
- `sample_frames`: decoded I/P/S/G/H/E frames
- `event_frames`: flight events (when present)
- `gps_track`: GPS coordinates (when present)

## Multi-log processing

```rust
use bbl_parser::{parse_bbl_file_all_logs, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let logs = parse_bbl_file_all_logs(Path::new("multi_flight.BBL"), ExportOptions::default(), false)?;
    for log in logs {
        println!("log {} of {} -> frames {}", log.log_number, log.total_logs, log.sample_frames.len());
    }
    Ok(())
}
```

## Parsing from memory

```rust
use bbl_parser::{parse_bbl_bytes, ExportOptions};

fn main() -> anyhow::Result<()> {
    let bytes = std::fs::read("flight.BBL")?;
    let log = parse_bbl_bytes(&bytes, ExportOptions::default(), false)?;
    println!("frames: {}", log.sample_frames.len());
    Ok(())
}
```

## Examples

Run the crate example that demonstrates multi-firmware support and PID extraction:

```bash
cargo build --example bbl_crate_test
cargo run --example bbl_crate_test -- flight.BBL
```

More details: [examples/README.md](./examples/README.md)

## Notes

- API is evolving while the project is WIP; names and structures may change.
- CSV field order and naming follow blackbox-tools to maximize compatibility.
- For CLI usage and high-level overview, see the main [README](./README.md).
