# Crate Usage: bbl_parser

Focused guidance for using the bbl_parser Rust crate.

## ⚠️ Important: Understanding Log Numbers and Flight Suffixes

A single BBL file can contain **multiple flight sessions** (separated by LOG_END events). The crate handles this with two different parsing functions:

| Function | Returns | Use Case | Output |
|----------|---------|----------|--------|
| `parse_bbl_file()` | First log only | Single-flight files or when you only need the first flight | No suffix (e.g., `flight.csv`) |
| `parse_bbl_file_all_logs()` | **All logs** | Multi-flight files or when you need all flights | With suffixes (e.g., `flight.01.csv`, `flight.02.csv`) |

**⚠️ Common mistake:** Using `parse_bbl_file()` on a multi-flight file will only export the first flight!

## Table of Contents
- [Installation](#installation)
- [Cargo features](#cargo-features)
- [Single-flight usage](#single-flight-usage)
- [Multi-flight usage](#multi-flight-usage)
- [Parsing from memory](#parsing-from-memory)
- [Export functionality](#export-functionality)
- [Flight numbering](#flight-numbering)
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

## Single-flight usage

For BBL files containing a single flight:

```rust
use bbl_parser::{parse_bbl_file, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let log = parse_bbl_file(Path::new("flight.BBL"), ExportOptions::default(), false)?;
    println!("firmware: {}", log.header.firmware_revision);
    println!("frames: {}", log.stats.total_frames);
    Ok(())
}
```

Key outputs on the BBLLog:
- `header`: configuration and metadata
- `frames`: decoded flight data frames
- `event_frames`: flight events (when present)
- `gps_track`: GPS coordinates (when present)
- `log_number` / `total_logs`: Current log number and total (useful to know if multi-log)

## Multi-flight usage

**For files with multiple flight sessions, ALWAYS use `parse_bbl_file_all_logs()`:**

```rust
use bbl_parser::{parse_bbl_file_all_logs, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let logs = parse_bbl_file_all_logs(Path::new("multi_flight.BBL"), ExportOptions::default(), false)?;
    
    for log in logs {
        println!("Flight {}/{}", log.log_number, log.total_logs);
        println!("  Frames: {}", log.stats.total_frames);
        println!("  Firmware: {}", log.header.firmware_revision);
    }
    Ok(())
}
```

### Best Practice: Handle Both Cases

To write robust code that works with any BBL file:

```rust
use bbl_parser::{parse_bbl_file_all_logs, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let logs = parse_bbl_file_all_logs(Path::new("flight.BBL"), ExportOptions::default(), false)?;
    
    // This works whether the file has 1 flight or many
    for log in logs {
        println!("Flight {}/{}: {} frames", log.log_number, log.total_logs, log.stats.total_frames);
        // Process this flight
    }
    Ok(())
}
```

## Parsing from memory

```rust
use bbl_parser::{parse_bbl_bytes, parse_bbl_bytes_all_logs, ExportOptions};

fn main() -> anyhow::Result<()> {
    let bytes = std::fs::read("flight.BBL")?;
    
    // Single flight (first only):
    let log = parse_bbl_bytes(&bytes, ExportOptions::default(), false)?;
    
    // All flights:
    let logs = parse_bbl_bytes_all_logs(&bytes, ExportOptions::default(), false)?;
    
    println!("frames: {}", log.stats.total_frames);
    Ok(())
}
```

## Export functionality

The crate provides full export capabilities for CSV, GPX, and Event data formats.

### CSV Export

Export parsed log data to CSV files (flight data + headers):

```rust
use bbl_parser::{parse_bbl_file_all_logs, export_to_csv, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let export_opts = ExportOptions {
        csv: true,
        gpx: false,
        event: false,
        output_dir: Some("output".to_string()),
        force_export: false,
    };
    
    // Export all logs from the file (handles both single and multi-log files)
    let logs = parse_bbl_file_all_logs(Path::new("flight.BBL"), export_opts.clone(), false)?;
    for log in logs {
        export_to_csv(&log, Path::new("flight.BBL"), &export_opts)?;
    }
    println!("CSV exported successfully");
    Ok(())
}
```

This creates two files per flight:
- `flight.csv` or `flight.01.csv`, `flight.02.csv`, etc. - Main flight data with blackbox_decode compatible format
- `flight.headers.csv` or `flight.01.headers.csv`, `flight.02.headers.csv`, etc. - Complete header information

**Flight Number Suffixes:**
- Single flight: No suffix (e.g., `flight.csv`)
- Multiple flights: Zero-padded 2-digit suffix (e.g., `flight.01.csv`, `flight.02.csv`, `flight.03.csv`)

### GPX Export

Export GPS data to GPX format for mapping applications:

```rust
use bbl_parser::{parse_bbl_file_all_logs, export_to_gpx, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let export_opts = ExportOptions {
        csv: false,
        gpx: true,
        event: false,
        output_dir: None,
        force_export: false,
    };
    
    let logs = parse_bbl_file_all_logs(Path::new("flight.BBL"), export_opts.clone(), false)?;
    
    for log in logs {
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

## Flight Numbering

Understanding how the crate handles flight numbers is critical for proper export handling:

### What Causes Multiple Flights?

A single BBL file contains multiple flights when the flight controller logs multiple sessions without restarting, typically separated by `LOG_END` events. Examples:
- Same drone, multiple flights in one session
- Interrupted logging (pause and resume)
- Extended flight with logging that resets internal counters

### Flight Number Behavior

| Scenario | Log Number | Total Logs | Output File | Notes |
|----------|-----------|-----------|------------|-------|
| Single flight | 1 | 1 | `flight.csv` | No suffix when only one log |
| 3 flights in file, export 1st | 1 | 3 | `flight.csv` | Using `parse_bbl_file()` only |
| 3 flights in file, export all | 1, 2, 3 | 3 | `flight.01.csv`, `flight.02.csv`, `flight.03.csv` | Using `parse_bbl_file_all_logs()` |

### Accessing Flight Information

```rust
use bbl_parser::{parse_bbl_file_all_logs, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let logs = parse_bbl_file_all_logs(
        Path::new("flight.BBL"), 
        ExportOptions::default(), 
        false
    )?;
    
    for log in logs {
        println!("Flight {}/{}", log.log_number, log.total_logs);
        println!("  Frames: {}", log.stats.total_frames);
        println!("  Firmware: {}", log.header.firmware_revision);
        
        // The export_to_csv function automatically adds the proper suffix
        // based on log.log_number and log.total_logs
    }
    Ok(())
}
```

### Suffix Rules

- **No suffix** if `total_logs == 1` (e.g., `flight.csv`)
- **Suffix** if `total_logs > 1` (e.g., `flight.01.csv`, `flight.02.csv`)
- **Format**: Zero-padded 2-digit number (`.01`, `.02`, ... `.99`)
- **Automatic**: The `export_to_csv()`, `export_to_gpx()`, and `export_to_event()` functions handle suffixing automatically

For runnable examples with complete code and output, see [examples/README.md](./examples/README.md).

## Examples

### Quick Start Examples

**Export single flight (or first flight only):**
```bash
cargo run --example csv_export -- flight.BBL ./output
```

**Export all flights with proper numbering:**
```bash
cargo run --example multi_flight_export -- flight.BBL ./output
```

**Complete parsing and data access:**
```bash
cargo run --example bbl_crate_test -- flight.BBL
```

More details: [examples/README.md](./examples/README.md)

## Notes

- API is evolving while the project is WIP; names and structures may change.
- CSV field order and naming follow blackbox-tools to maximize compatibility.
- For CLI usage and high-level overview, see the main [README](./README.md).
- **Always use `parse_bbl_file_all_logs()` in production code** to ensure all flights are processed correctly.
