# Crate Usage: bbl_parser

Focused guidance for using the bbl_parser Rust crate.

## Table of Contents
- [Installation](#installation)
- [Cargo features](#cargo-features)
- [Basic usage](#basic-usage)
- [Multi-log processing](#multi-log-processing)
- [Parsing from memory](#parsing-from-memory)
- [Export functionality](#export-functionality)
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

## Export functionality

The crate now provides full export capabilities for CSV, GPX, and Event data formats.

### CSV Export

Export parsed log data to CSV files (flight data + headers):

```rust
use bbl_parser::{parse_bbl_file, export_to_csv, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let export_opts = ExportOptions {
        csv: true,
        gpx: false,
        event: false,
        output_dir: Some("output".to_string()),
        force_export: false,
    };
    
    let log = parse_bbl_file(Path::new("flight.BBL"), export_opts.clone(), false)?;
    export_to_csv(&log, Path::new("flight.BBL"), &export_opts)?;
    println!("CSV exported successfully");
    Ok(())
}
```

This creates two files:
- `flight.csv` - Main flight data with blackbox_decode compatible format
- `flight.headers.csv` - Complete header information

### GPX Export

Export GPS data to GPX format for mapping applications:

```rust
use bbl_parser::{parse_bbl_file, export_to_gpx, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let export_opts = ExportOptions {
        csv: false,
        gpx: true,
        event: false,
        output_dir: None,
        force_export: false,
    };
    
    let log = parse_bbl_file(Path::new("flight.BBL"), export_opts.clone(), false)?;
    
    if !log.gps_coordinates.is_empty() {
        export_to_gpx(
            Path::new("flight.BBL"),
            0,  // log index
            log.total_logs,
            &log.gps_coordinates,
            &log.home_coordinates,
            &export_opts
        )?;
        println!("GPX exported successfully");
    }
    Ok(())
}
```

### Event Export

Export flight events to JSONL format:

```rust
use bbl_parser::{parse_bbl_file, export_to_event, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let export_opts = ExportOptions {
        csv: false,
        gpx: false,
        event: true,
        output_dir: None,
        force_export: false,
    };
    
    let log = parse_bbl_file(Path::new("flight.BBL"), export_opts.clone(), false)?;
    
    if !log.event_frames.is_empty() {
        export_to_event(
            Path::new("flight.BBL"),
            0,  // log index
            log.total_logs,
            &log.event_frames,
            &export_opts
        )?;
        println!("Events exported successfully");
    }
    Ok(())
}
```

### Complete Export Example

Export all formats at once:

```rust
use bbl_parser::{parse_bbl_file, export_to_csv, export_to_gpx, export_to_event, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let export_opts = ExportOptions {
        csv: true,
        gpx: true,
        event: true,
        output_dir: Some("output".to_string()),
        force_export: false,
    };
    
    let input_path = Path::new("flight.BBL");
    let log = parse_bbl_file(input_path, export_opts.clone(), false)?;
    
    // Export CSV
    export_to_csv(&log, input_path, &export_opts)?;
    
    // Export GPX if GPS data exists
    if !log.gps_coordinates.is_empty() {
        export_to_gpx(input_path, 0, log.total_logs, &log.gps_coordinates, &log.home_coordinates, &export_opts)?;
    }
    
    // Export events if event data exists
    if !log.event_frames.is_empty() {
        export_to_event(input_path, 0, log.total_logs, &log.event_frames, &export_opts)?;
    }
    
    println!("All exports completed successfully");
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
