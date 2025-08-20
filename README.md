# BBL Parser v0.9.0 (Work-in-Progress)

A high-performance Rust library and command-line tool for parsing BBL (Blackbox Log) files from flight controllers.

**Version:** 0.9.0 🚧 **Work-in-Progress**  
**Status:** Under active development  
**Supported Formats:** `.BBL`, `.BFL`, `.TXT` (case-insensitive) - Compatible with Betaflight, EmuFlight, and INAV  
**Library API:** Provides complete in-memory access to flight data, headers, GPS coordinates, and events

## Current Features

- **Rust Library API**: Complete programmatic access to BBL data structures in memory
- **Pure Rust Implementation**: Direct parsing logic without external dependencies
- **Universal File Support**: All common BBL formats with case-insensitive extension matching  
- **Complete Frame Support**: I, P, H, S, E, G frames with all encoding formats (SIGNED_VB, UNSIGNED_VB, NEG_14BIT, TAG8_8SVB, TAG2_3S32, TAG8_4S16)
- **Multi-Log Processing**: Automatic detection and processing of multiple flight logs within single files
- **Streaming Architecture**: Memory-efficient processing for large files (500K+ frames)
- **Advanced Frame Prediction**: Full predictor implementation for accurate P-frame decoding
- **CSV Export**: Flight data and header export with blackbox_decode compatibility
- **GPS Export**: GPX file generation for GPS-enabled flight logs
- **Event Export**: Flight event data extraction in JSONL format
- **Command Line Interface**: Glob patterns, debug mode, configurable output directories
- **Comprehensive Examples**: Practical demonstrations of crate usage with PID display and multi-firmware support

## Export Formats

### CSV Export (`--csv`)

Exports blackbox logs to CSV format with blackbox_decode compatibility:

- **`.XX.csv`**: Main flight data file containing I, P, S, G frame data
  - Field names header row in blackbox_decode compatible order
  - Time field labeled as "time (us)" for microsecond precision
  - All flight loop data (I frames) and status data (S frames) 
  - GPS data (G frames) when available
  - Time-sorted chronological data rows
- **`.XX.headers.csv`**: Complete header information file
  - Field,Value format with all configuration parameters
  - Frame definitions, system settings, firmware information
  - All BBL header metadata for analysis tools

### GPS Export (`--gpx`)

Exports GPS data to GPX format for mapping applications:

- **`.gps.gpx`**: GPS track file in standard GPX format
  - Geographic coordinates from GPS frames
  - Altitude information with proper firmware scaling
  - Timestamp data for track visualization
  - Compatible with Google Earth, GPS visualization tools

### Event Export (`--event`)

Exports flight events to JSONL format:

- **`.event`**: Flight event data in JSON Lines format
  - Individual JSON objects per line for streaming compatibility
  - Event types based on official Betaflight FlightLogEvent enum
  - Includes sync beeps, disarm events, flight mode changes, log boundaries
  - Compatible with log analysis tools expecting JSONL format

Where `XX` represents the flight log number (01, 02, 03, etc.) for multiple logs within a single BBL file.

**Example files generated:**
```
BTFL_LOG_20250601_121852.01.csv         # Flight data for log 1
BTFL_LOG_20250601_121852.01.headers.csv # Headers for log 1
BTFL_LOG_20250601_121852.gps.gpx        # GPS track data
BTFL_LOG_20250601_121852.event          # Flight events
BTFL_LOG_20250601_121852.02.csv         # Flight data for log 2  
BTFL_LOG_20250601_121852.02.headers.csv # Headers for log 2
```

## Installation & Usage

### As a Library (Rust Crate)

Add `bbl_parser` to your `Cargo.toml`:

```toml
[dependencies]
bbl_parser = { path = "path/to/bbl_parser" }
# or when published to crates.io:
# bbl_parser = "0.9.0"

# Optional features:
bbl_parser = { version = "0.9.0", features = ["serde", "json"] }
```

#### Crate Features

- **`csv`** (default): CSV export functionality
- **`json`**: JSON export support (requires `serde`)
- **`serde`**: Serialization support for data structures
- **`cli`** (default): Command line interface support

#### Basic Library Usage

```rust
use bbl_parser::{parse_bbl_file, ExportOptions, BBLLog};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Parse BBL file into memory structures
    let export_options = ExportOptions::default(); // No file exports
    let log = parse_bbl_file(
        Path::new("flight.BBL"), 
        export_options, 
        false // debug mode
    )?;
    
    // Access header information
    println!("Firmware: {}", log.header.firmware_revision);
    println!("Board: {}", log.header.board_info);
    println!("Craft: {}", log.header.craft_name);
    
    // Access frame data
    println!("Total frames: {}", log.sample_frames.len());
    for frame in &log.sample_frames {
        println!("Frame {}: {} fields at {}μs", 
                 frame.frame_type, 
                 frame.data.len(), 
                 frame.timestamp_us);
    }
    
    // Access GPS data
    for gps in &log.gps_coordinates {
        println!("GPS: lat={}, lon={}, alt={}m", 
                 gps.latitude, gps.longitude, gps.altitude);
    }
    
    // Access flight events
    for event in &log.event_frames {
        println!("Event type: {} at {}μs", 
                 event.event_type, event.timestamp_us);
    }
    
    Ok(())
}
```

#### Available Data Structures

The library exposes these main structures for 3rd party access:

> **Note:** This crate is currently undergoing systematic migration to a proper library API. Some data structures and field names may change in future versions as the migration completes.

**`BBLLog`** - Main container for all log data:
```rust
pub struct BBLLog {
    pub log_number: usize,           // Log number (1, 2, 3...)
    pub total_logs: usize,           // Total logs in file
    pub header: BBLHeader,           // Header/configuration data
    pub stats: FrameStats,           // Frame counts and timing
    pub sample_frames: Vec<DecodedFrame>, // Main flight data
    pub gps_coordinates: Vec<GpsCoordinate>, // GPS track points
    pub home_coordinates: Vec<GpsHomeCoordinate>, // Home position
    pub event_frames: Vec<EventFrame>, // Flight events
}
```

**`BBLHeader`** - Configuration and metadata:
```rust
pub struct BBLHeader {
    pub firmware_revision: String,   // "Betaflight 4.5.2 (024f8e13d)"
    pub board_info: String,          // "AXFL AXISFLYINGF7PRO"
    pub craft_name: String,          // "My Quad"
    pub data_version: u8,            // BBL format version
    pub looptime: u32,               // Main loop time in μs
    pub sysconfig: HashMap<String, i32>, // All system parameters
}
```

**`DecodedFrame`** - Individual flight data points:
```rust
pub struct DecodedFrame {
    pub frame_type: char,            // 'I', 'P', 'S', 'G', 'H', 'E'
    pub timestamp_us: u64,           // Time in microseconds
    pub data: HashMap<String, i32>,  // Field name -> value mapping
}
```

**`GpsCoordinate`** - GPS position data:
```rust
pub struct GpsCoordinate {
    pub latitude: f64,               // Decimal degrees
    pub longitude: f64,              // Decimal degrees  
    pub altitude: i32,               // Altitude in meters
    pub timestamp_us: u64,           // Time in microseconds
}
```

**`EventFrame`** - Flight events and state changes:
```rust
pub struct EventFrame {
    pub timestamp_us: u64,           // Time in microseconds
    pub event_type: u8,              // Event type ID
    pub event_name: String,          // Human-readable name
    pub data: Option<i32>,           // Optional event data
}
```

#### Multi-Log Processing

```rust
use bbl_parser::parse_bbl_file_all_logs;

// Parse file with multiple flight logs
let logs = parse_bbl_file_all_logs(
    Path::new("multi_flight.BBL"), 
    ExportOptions::default(), 
    false
)?;

for log in logs {
    println!("Processing log {} of {}", log.log_number, log.total_logs);
    println!("Duration: {:.1}s", log.duration_seconds());
    println!("Frames: {}", log.sample_frames.len());
}
```

#### Memory-Based Parsing

```rust
use bbl_parser::parse_bbl_bytes;

// Parse BBL data from memory (Vec<u8>, &[u8])
let bbl_data: Vec<u8> = std::fs::read("flight.BBL")?;
let log = parse_bbl_bytes(&bbl_data, ExportOptions::default(), false)?;
```

#### Examples

**BBL Crate Test Example** (`examples/bbl_crate_test`)

A comprehensive demonstration of the BBL parser crate featuring:
- Multi-firmware support (Betaflight, EmuFlight, iNav)  
- PID settings extraction with feedforward values
- Multi-log file processing with glob patterns
- Flight duration calculation and statistics

```bash
# Build the example
cargo build --example bbl_crate_test

# Run with a single file
cargo run --example bbl_crate_test -- flight.BBL

# Run with multiple files or patterns
cargo run --example bbl_crate_test -- logs/*.BBL *.bbl
```

See [`examples/README.md`](examples/README.md) for detailed usage instructions.

**Library API Examples**

See [`simple_data_access_example.rs`](simple_data_access_example.rs) and [`example_3rd_party_usage.rs`](example_3rd_party_usage.rs) for complete working examples of direct crate usage.

### As a Command Line Tool

```bash
git clone <repository-url>
cd bbl_parser
cargo build --release
```

**Note:** The main CLI tool provides CSV/GPX export and analysis. For a simpler demonstration of crate usage with PID display, see the `examples/bbl_crate_test` example above.

### Basic Usage
```bash
# Analysis and console statistics only
./target/release/bbl_parser file.BBL

# CSV export 
./target/release/bbl_parser --csv logs/*.BBL

# GPS export to GPX format (console stats + GPX file)
./target/release/bbl_parser --gpx logs/*.BBL

# Event data export (console stats + event file)
./target/release/bbl_parser --event logs/*.BBL

# All export formats
./target/release/bbl_parser --csv --gpx --event logs/*.BBL

# Multiple files and formats
./target/release/bbl_parser file1.BBL file2.BFL file3.TXT

# Glob patterns
./target/release/bbl_parser logs/*.{BBL,BFL,TXT}

# Debug mode
./target/release/bbl_parser --debug logs/*.BBL

# Custom output directory
./target/release/bbl_parser --csv --output-dir ./output logs/*.BBL
```

## Output

### Console Statistics (Always Displayed)

```
Processing: flight_log.BBL

Log 1 of 1, frames: 84235
Firmware: Betaflight 4.5.2 (024f8e13d) STM32F7X2
Board: AXFL AXISFLYINGF7PRO  
Craft: Volador 5

Statistics
Looptime         125 avg
I frames     1316
P frames    82845
H frames        1
G frames      833
E frames        4
S frames        6
Frames      84235
Data ver        2
```

### Export Output (When Enabled)

Additional output when export flags are used:
```
Exported GPS data to: flight_log.gps.gpx      # When --gpx used
Exported event data to: flight_log.event      # When --event used
```

### Debug Output

Debug mode adds frame data tables for detailed analysis:

```
=== FRAME DATA ===

I-frame data (25 frames):
     Index     Time(μs)     Loop accSmooth[ accSmooth[ gyroADC[0]  motor[0]  motor[1] ... (40 more fields)
         0            0        4          0          0         -5      1270      1270 ...
         1     36147802    71168       -163        130       2289      1260      1277 ...
       ...          ...      ... ... (18 frames skipped)
        23     36853826    73984       -332        -12       3512      1215      1210 ...
        24     36885919    74112       -430         26       3552      1205      1210 ...

P-frame data (50 frames):
     Index     Time(μs)     Loop accSmooth[ accSmooth[ gyroADC[0]  motor[0]  motor[1] ... (40 more fields)
         0 18446744073709551615        5        -11          9         27       632       637 ...
         1 18446744073709551615        6        -11          9         26       948       958 ...
       ...          ...      ... ... (18 frames skipped)
        49    939855786    71193        -75         94       1504       854       841 ...
```

**Debug mode** provides detailed analysis including:
- File size and binary data inspection
- Field definitions and encoding details  
- **Frame data tables** organized by type (I, P, S, G, E, H frames)
- Smart sampling: shows all frames ≤30, or first 5 + middle 5 + last 5 when >30 frames

## Frame Support & Compatibility

**Frame Types:** I, P, H, S, E, G frames  
**Encoding:** All major BBL formats (SIGNED_VB, UNSIGNED_VB, NEG_14BIT, TAG8_8SVB, TAG2_3S32, TAG8_4S16)  
**Predictors:** Reference-compliant implementation for P-frame decoding

### Event Frame Support

Event parsing uses the official Betaflight FlightLogEvent enum:
- **Type 0**: Sync beep (initialization)
- **Type 15**: Disarm event 
- **Type 30**: Flight mode change
- **Type 255**: Log end marker
- **Additional types**: Autotune, inflight adjustment, logging resume events

## Performance & Limitations

**⚠️ Development Status**: This is work-in-progress software with the following limitations:
- Limited testing with GPS and Event frame processing
- May have compatibility issues with some specialized log formats
- Performance optimizations still in development
- API may change between versions

**Current Capabilities:**
- Extensively tested with Betaflight and EmuFlight log files
- Memory-efficient streaming architecture for large files
- Processes files that may cause external decoders to fail
- Zero external dependencies - no blackbox_decode tools required

## Dependencies

- `clap` (v4.0+) - CLI parsing
- `glob` (v0.3) - File patterns  
- `anyhow` (v1.0) - Error handling

## Betaflight Firmware Compatibility

**Flight Mode Flags, State Flags, and Failsafe Phases:** This parser outputs data that matches current Betaflight firmware specifications exactly.

- **Flight Mode Flags**: Current `flightModeFlags_e` enum from Betaflight runtime_config.h
  - Supports: ANGLE_MODE, HORIZON_MODE, MAG, BARO, GPS_HOLD, HEADFREE, PASSTHRU, FAILSAFE_MODE, GPS_RESCUE_MODE
  - Output format: `"ANGLE_MODE|HORIZON_MODE"` (pipe-separated for CSV compatibility)
  - Includes GPS_RESCUE_MODE flag (bit 11) from current firmware

- **State Flags**: Current `stateFlags_t` enum from Betaflight runtime_config.h  
  - Supports: GPS_FIX_HOME, GPS_FIX, CALIBRATE_MAG, SMALL_ANGLE, FIXED_WING
  - Output format: `"GPS_FIX_HOME|GPS_FIX"` (pipe-separated for CSV compatibility)

- **Failsafe Phase**: Current `failsafePhase_e` enum from Betaflight failsafe.h
  - Supports: IDLE, RX_LOSS_DETECTED, LANDING, LANDED, RX_LOSS_MONITORING, RX_LOSS_RECOVERED, GPS_RESCUE
  - Includes phases 4-6 from current firmware

**Reference Compatibility:** Verified against blackbox-tools and current Betaflight master branch.

## Overview

- [GOALS.md](./GOALS.md)
- [FRAMES.md](./FRAMES.md)
- [OVERVIEW.md](./OVERVIEW.md)

## Contributing

**⚠️ Development Project**: This is work-in-progress software. Contributions welcome with understanding that:
- APIs may change between versions
- Not all edge cases have been tested
- Performance optimizations are ongoing

**Priority Areas:** Testing across firmware versions, file format compatibility, performance optimization

## License

This project is provided as-is for educational and development use with flight controller blackbox logs.  

## Acknowledgments

Based on [Betaflight Blackbox Log Viewer](https://github.com/betaflight/blackbox-log-viewer) and [blackbox-tools](https://github.com/betaflight/blackbox-tools)
