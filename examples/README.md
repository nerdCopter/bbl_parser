# BBL Parser Examples

This directory contains example programs demonstrating how to use the `bbl_parser` crate.

## Quick Start

### Single Flight Export (First Flight Only)
```bash
cargo run --example csv_export -- flight.BBL ./output
```

### Multiple Flights Export (All Flights with Numbering)
```bash
cargo run --example multi_flight_export -- flight.BBL ./output
```

## Available Examples

### 1. csv_export.rs ⭐ **START HERE**
**Purpose:** Export the first flight/log from a BBL file to CSV format.

- **Use this for:** Single-flight files or when you only need the first flight
- **API:** `parse_bbl_file()` - Returns only the first log
- **Output:** Single `.csv` file (no suffix)
- **Time:** Fast, processes only one flight

**Important Note:** If your BBL file contains multiple flight sessions (separated by LOG_END events), this example will only export the first one. See `multi_flight_export.rs` for handling multiple flights.

```bash
cargo run --example csv_export -- flight.BBL ./output
```

### 2. multi_flight_export.rs ⭐ **Use for Multi-Session Files**
**Purpose:** Export ALL flights/logs from a BBL file to CSV with proper numbering.

- **Use this for:** BBL files with multiple flight sessions
- **API:** `parse_bbl_file_all_logs()` - Returns all logs
- **Output:** Multiple files with suffixes: `.01.csv`, `.02.csv`, `.03.csv`, etc.
- **Flight Numbering:** Automatic 2-digit zero-padded suffix based on log count

**Example Output:**
```
Flight 1/3:
  Frames: 1,787
  ✓ Exported as .01.csv

Flight 2/3:
  Frames: 61,714
  ✓ Exported as .02.csv

Flight 3/3:
  Frames: 181,554
  ✓ Exported as .03.csv
```

```bash
cargo run --example multi_flight_export -- flight.BBL ./output
```

### 3. bbl_crate_test
**Purpose:** Comprehensive parsing and data access demonstration with multi-log support.

- **Features:** File pattern matching, debug output, PID settings display
- **Use this for:** Understanding full crate API and data structures
- **Multi-Log Support:** Handles files containing multiple flight logs

```bash
cargo run --example bbl_crate_test -- flight.BBL
cargo run --example bbl_crate_test -- *.BBL  # Process multiple files
```

### 4. export_demo

See [export_demo Example](#export_demo-example) section below for comprehensive details on CSV, GPX, and Event export functionality.

## Understanding Flight Numbers

A single BBL file can contain **multiple flight sessions**, separated by `LOG_END` events. When this happens:

| Scenario | Function | Output |
|----------|----------|--------|
| Single flight | `parse_bbl_file()` | `flight.csv` (no suffix) |
| Single flight via all_logs | `parse_bbl_file_all_logs()` | `flight.csv` (no suffix) |
| Multiple flights | `parse_bbl_file()` | Only exports 1st: `flight.csv` |
| Multiple flights | `parse_bbl_file_all_logs()` | All flights: `flight.01.csv`, `flight.02.csv`, etc. |

## API Pattern: Which Function to Use?

### For Crate Users (Library Integration)

```rust
use bbl_parser::{parse_bbl_file, parse_bbl_file_all_logs, export_to_csv, ExportOptions};

// If you only care about the first flight:
let log = parse_bbl_file(path, options, false)?;
export_to_csv(&log, path, &options)?;

// If you need to handle all flights:
let logs = parse_bbl_file_all_logs(path, options, false)?;
for log in logs {
    export_to_csv(&log, path, &options)?;
    // Library automatically handles .01, .02, .03 suffixes
}
```

### For CLI Applications

Use `parse_bbl_file_all_logs()` to ensure all flight data is processed, not just the first one.

## Key Features

- **File Input Support**: Accepts BBL, BFL, and TXT files (case-insensitive)
- **Glob Pattern Support**: Process multiple files with wildcards
- **Flight Information Display**: Shows firmware, version, duration, and frame statistics  
- **Multi-Log Support**: Handles files containing multiple flight logs with automatic suffixing
- **Clean Output**: Focused, essential information only

## Implementation Notes

These examples demonstrate:

1. **Crate Usage**: How to import and use the `bbl_parser` crate
2. **File Handling**: Case-insensitive file extension matching and glob pattern support
3. **Data Access**: Accessing headers, frames, GPS, and events
4. **Error Handling**: Proper error handling with the `anyhow` crate
5. **Multi-Log Processing**: Correctly handling BBL files with multiple flight sessions
6. **Export API**: CSV, GPX, and event export functionality
7. **Flight Numbering**: Automatic suffix generation for multi-flight files

## Common Mistakes to Avoid

❌ **WRONG:** Use `parse_bbl_file()` for multi-flight files expecting all flights
```rust
let log = parse_bbl_file(path, options, false)?;  // Only gets first flight!
```

✅ **CORRECT:** Use `parse_bbl_file_all_logs()` to get all flights
```rust
let logs = parse_bbl_file_all_logs(path, options, false)?;
for log in logs {
    export_to_csv(&log, path, &options)?;
}
```

---

## export_demo Example

Demonstrates the complete export API for CSV, GPX, and Event formats.

### Features

- **CSV Export**: Exports flight data and headers in blackbox_decode compatible format
- **GPX Export**: Converts GPS data to standard GPX format for mapping applications
- **Event Export**: Exports flight events to JSONL format
- **Multi-format Export**: Exports all formats simultaneously
- **Output Directory**: Configurable output directory support

### Usage

```bash
# Build the example
cargo build --example export_demo

# Export to current directory
cargo run --example export_demo -- flight.BBL

# Export to specific directory
cargo run --example export_demo -- flight.BBL ./output
```

### Example Output

```
=== BBL Parser Export Demo ===
Input file: flight.BBL
Output directory: ./output

Parsing BBL file...

=== Log Information ===
Firmware: Betaflight 4.5.1 (77d01ba3b) STM32F7X2
Board: MAMBAF722
Craft: My Quad
Data version: 2
Looptime: 125 μs

=== Frame Statistics ===
Total frames: 84235
I frames: 1316
P frames: 82845
S frames: 6
G frames: 833
H frames: 1
E frames: 4

Duration: 10.53s (10529375 μs)

=== Exporting Data ===
Exporting CSV files...
✓ CSV export complete
Exporting GPX file (833 GPS coordinates)...
✓ GPX export complete
Exporting event file (4 events)...
✓ Event export complete

=== Sample Events ===
  1. Sync beep (time: 0 μs)
  2. Disarm (time: 10529375 μs)
  ... and 2 more events

=== Export Complete ===
All requested exports completed successfully!
```

### Implementation Notes

This example demonstrates:

1. **Export API Usage**: How to use all three export functions
2. **ExportOptions Configuration**: Setting up export options
3. **Conditional Export**: Only exporting GPS/Events when data exists
4. **Error Handling**: Proper error handling for file operations
5. **User Feedback**: Progress indication and result reporting

### Exported Files

When run, this example creates:
- `flight.csv` - Main flight data (I, P frames)
- `flight.headers.csv` - Complete header information
- `flight.gps.gpx` - GPS track in GPX format (if GPS data exists)
- `flight.event` - Flight events in JSONL format (if events exist)

For multi-log files, outputs are numbered:
- `flight.01.csv`, `flight.02.csv`, etc.
- `flight.01.gps.gpx`, `flight.02.gps.gpx`, etc.
- `flight.01.event`, `flight.02.event`, etc.

---

## Additional Export Examples

Four more specialized examples provide focused demonstrations of individual export functionality:

### csv_export - CSV Export Only

See [1. csv_export.rs](#1-csv_exportrs--start-here) under [Available Examples](#available-examples) for full documentation.

### gpx_export - GPS Data Export

**File:** `examples/gpx_export.rs`

Demonstrates GPS export to GPX format for mapping applications.

**Usage:**
```bash
cargo run --example gpx_export --release -- flight.BBL ./output
```

**Status:** ⏳ Partially implemented - GPX export function is ready, but GPS data collection in parser module requires enhancement. Use CLI: `bbl_parser --gps flight.BBL`

### event_export - Flight Event Export

**File:** `examples/event_export.rs`

Demonstrates flight event export in JSONL format.

**Usage:**
```bash
cargo run --example event_export --release -- flight.BBL ./output
```

**Status:** ⏳ Partially implemented - Event export function is ready, but event data collection in parser module requires enhancement. Use CLI: `bbl_parser --event flight.BBL`

### multi_export - All Formats

**File:** `examples/multi_export.rs`

Demonstrates comprehensive export of all available formats with detailed statistics and conditional export based on data availability.

**Usage:**
```bash
cargo run --example multi_export --release -- flight.BBL ./output
```

**Status:** ✅ Fully functional for CSV, ⏳ GPS/Event pending parser enhancement

## Testing All Examples

```bash
# Test CSV export
cargo run --example csv_export --release -- input/BTFL_Gonza_2.5_Cine_FLipsandrolls.BBL /tmp/test

# Test with multiple files
cargo run --example multi_export --release -- input/BTFL_KWONGKAN_10inch_0326_00_Filter.BBL /tmp/test

# Test all examples
for example in csv_export gpx_export event_export multi_export; do
  cargo run --example $example --release -- input/BTFL_Gonza_2.5_Cine_FLipsandrolls.BBL /tmp/test
done
```

## Current Implementation Status

| Example | CSV | GPX | Event | Status |
|---------|-----|-----|-------|--------|
| csv_export | ✅ | — | — | Production Ready |
| gpx_export | — | ⏳* | — | API Ready* |
| event_export | — | — | ⏳* | API Ready* |
| multi_export | ✅ | ⏳* | ⏳* | Partially Ready |
| export_demo | ✅ | ⏳* | ⏳* | Partially Ready |

*GPS/Event functions implemented and working, but require parser module enhancement to collect data during parsing.

## API Integration

All export functions are accessible via the crate. These examples show the API in use:

### CSV Export
```rust
use bbl_parser::{parse_bbl_file, export_to_csv, ExportOptions};
use std::path::Path;

let opts = ExportOptions { csv: true, gpx: false, event: false, output_dir: None, force_export: false };
let log = parse_bbl_file(Path::new("flight.BBL"), opts.clone(), false)?;
export_to_csv(&log, Path::new("flight.BBL"), &opts)?;
// Creates: flight.csv + flight.headers.csv
```

### GPX + Event Export
```rust
use bbl_parser::{export_to_gpx, export_to_event, ExportOptions};

let opts = ExportOptions { csv: false, gpx: true, event: true, output_dir: Some("out".into()), force_export: false };

if !log.gps_coordinates.is_empty() {
    export_to_gpx(Path::new("flight.BBL"), 0, 1, &log.gps_coordinates, &log.home_coordinates, &opts)?;
}

if !log.event_frames.is_empty() {
    export_to_event(Path::new("flight.BBL"), 0, 1, &log.event_frames, &opts)?;
}
```

See `CRATE_USAGE.md` for basic setup and API reference.
