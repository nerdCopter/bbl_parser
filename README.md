# BBL Parser - Rust Implementation

A Rust implementation of BBL (Blackbox Log) parser based on the official reference implementations from the Betaflight blackbox-log-viewer and the blackbox-tools repositories.

**Supported Formats:** `.BBL`, `.BFL`, `.TXT` (case-insensitive) - Compatible with Betaflight, EmuFlight, and INAV

## Features

- **Pure Rust Implementation**: Direct parsing logic without external tools
- **Universal File Support**: Common BBL formats with case-insensitive extension matching
- **Complete Frame Support**: I, P, H, S, E, G frames with all encoding formats (SIGNED_VB, UNSIGNED_VB, NEG_14BIT, TAG8_8SVB, TAG2_3S32, TAG8_4S16)
- **Multi-Log Processing**: Detects and processes multiple flight logs within single files
- **Streaming Architecture**: Memory-efficient processing for large files (500K+ frames)
- **Frame Prediction**: Full predictor implementation (PREVIOUS, STRAIGHT_LINE, AVERAGE_2, MINTHROTTLE, etc.)
- **CSV Export**: Export flight data to CSV format with separate header files for H frames
- **Command Line Interface**: Glob patterns, debug mode, configurable output directories
- **Debug Frame Data**: Detailed frame-by-frame data display with smart sampling (first/middle/last when >30 frames)
- **High Performance**: Reference-equivalent accuracy (100.02%), superior file compatibility (91.3% vs 43.5% success rate)

## CSV Export Format

The `--csv` option exports blackbox logs to CSV format with Betaflight-compatible field ordering:

- **`.XX.csv`**: Main flight data file containing I, P, S, G frame data
  - Field names header row in the same order as Betaflight blackbox-log-viewer
  - Field names are trimmed of leading/trailing spaces
  - Time field labeled as "time (us)" to indicate microsecond units
  - I frame fields first (main flight loop data)
  - S frame fields second (slow/status data)  
  - G frame fields third (GPS data, excluding duplicate time field)
  - Time-sorted data rows with all blackbox fields
- **`.XX.headers.csv`**: Plaintext headers file containing all BBL header information
  - Field,Value format with all configuration parameters
  - Includes frame definitions, system settings, firmware info, etc.

Where `XX` represents the flight log number (01, 02, 03, etc.) for multiple logs within a single BBL file.

**Example CSV files generated:**
```
BTFL_LOG_20250601_121852.01.csv         # Flight data for log 1
BTFL_LOG_20250601_121852.01.headers.csv # Plaintext headers for log 1
BTFL_LOG_20250601_121852.02.csv         # Flight data for log 2  
BTFL_LOG_20250601_121852.02.headers.csv # Plaintext headers for log 2
BTFL_LOG_20250601_121852.03.csv         # Flight data for log 3
BTFL_LOG_20250601_121852.03.headers.csv # Plaintext headers for log 3
```
## Installation & Usage

```bash
git clone <repository-url>
cd bbl_parser
cargo build --release
```

### Basic Usage
```bash
# Single file
./target/release/bbl_parser file.BBL

# Multiple files and formats
./target/release/bbl_parser file1.BBL file2.BFL file3.TXT

# Glob patterns
./target/release/bbl_parser logs/*.{BBL,BFL,TXT}

# Debug mode
./target/release/bbl_parser --debug logs/*.BBL

# CSV export 
./target/release/bbl_parser --csv logs/*.BBL

# CSV export to specific directory
./target/release/bbl_parser --csv --output-dir ./csv_output logs/*.BBL
```

## Output

```
Processing: flight_log.BBL

Log 1 of 1, frames: 1410
Firmware: Betaflight 4.5.1 (77d01ba3b) STM32F7X2
Board: DIAT MAMBAF722_2022B

Statistics
Looptime         125 avg
I frames          25
P frames        1352
H frames          22
E frames           2
S frames           9
Frames         1410
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

## Architecture

**`src/main.rs`** - CLI, file handling, header parsing, statistics  
**`src/bbl_format.rs`** - BBL binary format, encoding/decoding, frame parsing

**Frame Support:** I, P, H, S, E, G frames | **Encoding:** All major BBL formats | **Predictors:** JavaScript-compliant implementation
## Development Status

**Near Production Ready:** Header parsing, frame decoding, multi-log support, streaming processing, CLI with glob patterns, CSV export

**Testing Complete:** 91.3% file success rate across 23 BBL files, reference-equivalent accuracy (100.02%) based on tested file subset

**Remaining Work:** Code refinement (replace unwrap() calls), complete missing implementations, expand error handling

**Implementation:** Direct port of [Betaflight blackbox-log-viewer](https://github.com/betaflight/blackbox-log-viewer) JavaScript reference along with analysis of the [blackbox-tools](https://github.com/betaflight/blackbox-tools) C reference.

## Dependencies

- `clap` (v4.0+) - CLI parsing
- `glob` (v0.3) - File patterns  
- `anyhow` (v1.0) - Error handling

## Testing

```bash
# Basic test
./target/release/bbl_parser flight_log.BBL

# Multi-format test
./target/release/bbl_parser --debug log1.BBL log2.BFL log3.TXT

# Large file test
timeout 60s ./target/release/bbl_parser logs/*.BBL
```

## Betaflight Firmware Compatibility

✅ **Firmware-Accurate Flag Formatting**: This parser outputs flight mode flags, state flags, and failsafe phases that match the current Betaflight firmware exactly.

- **Flight Mode Flags**: Uses the correct `flightModeFlags_e` enum from Betaflight runtime_config.h
  - Supports current modes: ANGLE_MODE, HORIZON_MODE, MAG, BARO, GPS_HOLD, HEADFREE, PASSTHRU, FAILSAFE_MODE, GPS_RESCUE_MODE
  - Output format: `"ANGLE_MODE|HORIZON_MODE"` (pipe-separated for CSV compatibility)
  - Includes new GPS_RESCUE_MODE flag (bit 11) from current firmware

- **State Flags**: Uses the correct `stateFlags_t` enum from Betaflight runtime_config.h  
  - Supports: GPS_FIX_HOME, GPS_FIX, CALIBRATE_MAG, SMALL_ANGLE, FIXED_WING
  - Output format: `"GPS_FIX_HOME|GPS_FIX"` (pipe-separated for CSV compatibility)

- **Failsafe Phase**: Uses the correct `failsafePhase_e` enum from Betaflight failsafe.h
  - Supports all phases: IDLE, RX_LOSS_DETECTED, LANDING, LANDED, RX_LOSS_MONITORING, RX_LOSS_RECOVERED, GPS_RESCUE
  - Includes new phases 4-6 from current firmware

**Compatibility**: Verified against [blackbox-tools pull request #60](https://github.com/betaflight/blackbox-tools/pull/60) and current Betaflight master branch.

## Overview

- [GOALS.md](./GOALS.md)
- [FRAMES.md](./FRAMES.md)
- [OVERVIEW.md](./OVERVIEW.md)

## Contributing

**Priority Areas:** CSV export, GPS frame parsing, advanced statistics, performance optimization

**Guidelines:** Follow JavaScript reference, test all file extensions (.BBL, .BFL, .TXT), maintain clean architecture

**Development Note:** This project includes `.github/copilot-instructions.md` which has been utilized during development and may require future improvement for enhanced AI assistance.

## License

This project is provided as-is for educational and practical use with flight controller blackbox logs.

## Acknowledgments

Based on [Betaflight Blackbox Log Viewer](https://github.com/betaflight/blackbox-log-viewer)
