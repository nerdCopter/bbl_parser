# BBL Parser Examples

This directory contains example programs demonstrating how to use the `bbl_parser` crate.

## Available Examples

### 1. bbl_crate_test
Basic parsing and data access demonstration.

### 2. export_demo
Complete export functionality demonstration (CSV, GPX, Event).

## bbl_crate_test Example

## Features

- **File Input Support**: Accepts BBL, BFL, and TXT files (case-insensitive)
- **Glob Pattern Support**: Process multiple files with wildcards
- **Flight Information Display**: Shows firmware, version, duration, and frame statistics  
- **PID Settings**: Displays PID controller settings from log headers (always shown)
- **Multi-Log Support**: Handles files containing multiple flight logs
- **Clean Output**: Focused, essential information only

## Usage

### Basic Usage
```bash
# Build the example
cargo build --example bbl_crate_test

# Process single file
cargo run --example bbl_crate_test -- flight.BBL

# Process multiple files
cargo run --example bbl_crate_test -- file1.BBL file2.BFL file3.TXT

# Use glob patterns (case-insensitive)
cargo run --example bbl_crate_test -- *.BBL *.bbl
cargo run --example bbl_crate_test -- logs/*.{BBL,BFL,TXT}
```

### Command Line Options
```bash
# Enable debug output from parser
cargo run --example bbl_crate_test -- --debug flight.BBL

# Process multiple files
cargo run --example bbl_crate_test -- logs/*.BBL
```

## Example Output

```
Processing: flight_log.BBL
  Firmware: EmuFlight 0.4.3 (b5690ecef) FOXEERF722V4
  Craft: My Racing Quad
  Flight Duration: 67.5 seconds
  PID Settings:
    Roll: P=100, I=80, D=100
    Pitch: P=100, I=80, D=100
    Yaw: P=100, I=70, D=100
```

## Implementation Notes

This test program demonstrates:

1. **Crate Usage**: Shows how to import and use the `bbl_parser` crate
2. **File Handling**: Case-insensitive file extension matching and glob pattern support
3. **Data Access**: Accessing all major data structures (headers, frames, GPS, events)
4. **Error Handling**: Proper error handling with the `anyhow` crate
5. **CLI Interface**: Command-line argument parsing with `clap`

## Crate API Demonstration

The program showcases these key crate features:

- `parse_bbl_file_all_logs()` - Parse multiple logs from a single file
- `BBLLog` structure access - Header, frames, GPS, events
- `ExportOptions::default()` - Memory-only parsing without file exports
- Header configuration access - PID settings and system parameters
- Frame statistics and timing calculations

This serves as both a functional tool and a reference implementation for using the `bbl_parser` crate in other projects.

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

**File:** `examples/csv_export.rs`

Demonstrates basic CSV export functionality. Creates two CSV files for every flight:
- Flight data CSV with all sensor readings
- Headers CSV with complete configuration

**Usage:**
```bash
cargo run --example csv_export --release -- flight.BBL ./output
```

**Status:** ✅ Fully functional

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

All export functions are accessible via the crate:

```rust
use bbl_parser::{
    parse_bbl_file, 
    export_to_csv, 
    export_to_gpx, 
    export_to_event,
    ExportOptions
};
```

See `CRATE_USAGE.md` in the root directory for comprehensive integration examples.
