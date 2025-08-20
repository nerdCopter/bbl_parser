# BBL Parser Example

This example demonstrates how to use the `bbl_parser` crate to parse and display information from BBL (Blackbox Log) files.

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
