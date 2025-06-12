# BBL Parser - Rust Implementation

A Rust implementation of BBL (Blackbox Log) parser based on the official JavaScript reference implementation from the Betaflight blackbox-log-viewer repository.

## Project Status

✅ **COMPLETED:**
- **Header Parsing**: Complete BBL header parsing including firmware info, field definitions, and frame specifications
- **Binary Frame Decoding**: Implements major encoding formats (SIGNED_VB, UNSIGNED_VB, NEG_14BIT, TAG8_8SVB, TAG2_3S32, TAG8_4S16)
- **Multi-Frame Support**: Parses I, P, H, S, E frames with basic G frame support
- **Command Line Interface**: Full CLI with glob pattern support, debug mode, and file processing
- **Statistics Output**: Frame counts and basic analysis similar to JavaScript version
- **Clean Architecture**: Modular design with `main.rs` and `bbl_format.rs` separation

🔄 **PARTIALLY IMPLEMENTED:**
- **CSV Export**: CLI flag exists, basic structure in place
- **GPS Frame Decoding**: Basic parsing implemented, full decoding pending
- **Multi-Log Support**: Structure exists but currently processes single logs per file
- **Predictor Application**: Framework implemented but not fully utilized

🚧 **IN PROGRESS:**
- **Complete Frame Reconstruction**: Field prediction and delta frame application
- **Advanced Statistics**: Timing analysis, data rates, missing iteration calculations  
- **Enhanced Error Handling**: Improved parsing robustness and recovery
- **Performance Optimization**: Large file handling and memory efficiency

## Goals

This project implements BBL (Blackbox Log) binary format specification by replicating the JavaScript reference implementation from:
- https://github.com/betaflight/blackbox-log-viewer/tree/master/src
- Key reference files: `flightlog.js`, `flightlog_parser.js`, `datastream.js`, `decoders.js`

**Current Implementation Status**: The parser successfully reads BBL headers, decodes binary frame data, and provides statistics output. Frame parsing supports major encoding formats with basic field reconstruction.

**Important**: This implementation does NOT call external `blackbox_decode_*` binaries. It implements the parsing logic directly in Rust following the JavaScript reference.

## Features

### Currently Working
- **Pure Rust Implementation**: No external dependencies on blackbox_decode_* tools
- **Header Analysis**: Complete extraction of firmware configuration and flight settings
- **Binary Frame Parsing**: Supports I, P, H, S, E frames with multiple encoding formats
- **Command Line Interface**: Easy-to-use CLI with debug output and file pattern matching
- **Multi-File Support**: Process single files or use glob patterns for batch processing
- **Statistics Output**: Frame counts and basic parsing statistics

### Encoding Support
- **SIGNED_VB / UNSIGNED_VB**: Variable byte encoding for compact data representation
- **NEG_14BIT**: Negative 14-bit encoding for specific sensor data
- **TAG8_8SVB**: Tag-based encoding for efficient delta compression
- **TAG2_3S32**: 2-bit tag with 3 signed 32-bit values
- **TAG8_4S16**: 8-bit tag with 4 signed 16-bit values
- **NULL**: No-data encoding for unused fields

### In Development
- **Complete CSV Export**: Framework exists, implementation in progress
- **GPS Frame Decoding**: Basic support implemented, full decoding pending
- **Advanced Predictors**: Field prediction and delta frame reconstruction
- **Performance Optimization**: Large file handling and memory efficiency

## Installation

Ensure you have Rust installed (https://rustup.rs/), then build the project:

```bash
git clone <repository-url>
cd bbl_parser_from_javascript
cargo build --release
```

The compiled binary will be available at `./target/release/bbl_parser`.

## Usage

### Basic Usage

Parse a single BBL file:
```bash
./target/release/bbl_parser file.BBL
```

Parse multiple files:
```bash
./target/release/bbl_parser file1.BBL file2.BBL file3.BBL
```

Use glob patterns:
```bash
./target/release/bbl_parser input/*.BBL
./target/release/bbl_parser "logs/flight_*.BBL"
```

### Debug Mode

Enable detailed parsing information and debug output:
```bash
./target/release/bbl_parser --debug input/*.BBL
```

### CSV Export (In Development)

Enable CSV export of frame data:
```bash
./target/release/bbl_parser --csv input/*.BBL
```

### Help

```bash
./target/release/bbl_parser --help
```

## Output

The parser provides comprehensive statistics and analysis:

### Current Output Features:
- **Log Information**: Log numbering and frame counts
- **Header Information**: Firmware type, version, board info, craft name
- **Frame Statistics**: Breakdown by frame type (I, P, H, S, E, G frames)
- **Configuration**: Looptime and other key settings
- **Debug Information**: Header details and parsing progress (with --debug flag)

### Example Output

```
Processing: BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.BBL

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

### With Debug Output

Debug mode provides additional information:
- File size and header count
- Field definitions for each frame type
- Binary data inspection
- Frame parsing progress
- Encoding details and field mappings

### Future Output Enhancements

Planned additions to match JavaScript blackbox-log-viewer output:
- **Timing Analysis**: Start/end times, duration in MM:SS.sss format
- **Performance Metrics**: Data rate in Hz, bytes/sec, and baud rate
- **Error Analysis**: Failed frame detection and missing iteration estimates
- **Advanced Statistics**: Sensor data analysis and loop iteration tracking

## Architecture

The parser consists of two main modules with clean separation of concerns:

### Core Modules

**`src/main.rs` (823 lines)**
- Command line interface and argument parsing
- File handling and glob pattern processing  
- High-level BBL parsing coordination
- Header parsing and frame definition extraction
- Statistics calculation and output formatting
- Frame parsing orchestration using bbl_format module

**`src/bbl_format.rs` (544 lines)**
- BBL binary format implementation following JavaScript reference
- `BBLDataStream` for efficient binary data reading
- Low-level encoding/decoding functions for all supported formats
- Sign extension utilities and bit manipulation
- Frame parsing primitives and data extraction

### Key Data Structures

- **`BBLHeader`**: Complete header information including frame definitions
- **`FrameDefinition`**: Field specifications with encoding, predictor, and type info
- **`FrameStats`**: Parsing statistics and frame counts
- **`DecodedFrame`**: Individual frame data with timestamp and field values
- **`BBLDataStream`**: Binary data stream abstraction for reading encoded data

### BBL Format Support

**Current Frame Type Support:**
- **I Frames (Intra)**: Full data frames with complete sensor readings - ✅ Implemented
- **P Frames (Inter)**: Delta frames with differential data - ✅ Implemented  
- **S Frames (Slow)**: Flight mode and status information - ✅ Implemented
- **H Frames (Home)**: Reference/home position data - ✅ Basic support
- **E Frames (Event)**: Special events and markers - ✅ Basic support
- **G Frames (GPS)**: GPS coordinate and navigation data - 🔄 Partial support

**Encoding Format Support:**
- Variable byte encoding (signed/unsigned) - ✅ Complete
- TAG-based compression formats - ✅ Complete
- 14-bit negative encoding - ✅ Complete
- NULL encoding for unused fields - ✅ Complete

## Current Limitations & Development Status

### Working Features ✅
- Header parsing and frame definition extraction
- Basic frame parsing for I, P, S, H, E frames
- Multiple encoding format support
- Command line interface with glob patterns
- Debug output and parsing statistics
- Multi-file processing capabilities

### Known Limitations 🔄
- **Predictor Application**: Framework exists but not fully utilized for field reconstruction
- **CSV Export**: CLI flag implemented, file writing needs completion
- **GPS Frame Decoding**: Basic parsing works, full field extraction pending
- **Multi-Log Files**: Structure supports it but currently processes single logs
- **Advanced Statistics**: Missing timing analysis and data rate calculations
- **Error Recovery**: Limited robustness during malformed frame parsing

### Compilation Notes
- Builds successfully with `cargo build --release`
- Produces warnings for unused fields/constants (by design for future features)
- All warnings are for development scaffolding, no errors present
- Binary name: `bbl_parser` (not `bbl_parser_from_javascript`)

## Implementation Fidelity

This implementation closely follows the JavaScript reference from the Betaflight blackbox-log-viewer:
- **Header parsing**: Direct port of field definition extraction logic
- **Encoding functions**: Exact replicas of JavaScript encoding/decoding algorithms  
- **Frame structure**: Maintains same frame type hierarchy and field organization
- **Data stream handling**: Similar buffered reading approach with position tracking

**Differences from JavaScript version:**
- Rust's type safety eliminates many runtime errors possible in JavaScript
- Memory management is automatic and more efficient
- Error handling uses Result types instead of exceptions
- Some JavaScript-specific optimizations replaced with Rust idioms

## Dependencies

- **`clap`** (v4.0+) - Command line argument parsing with derive features
- **`glob`** (v0.3) - File pattern matching and expansion
- **`anyhow`** (v1.0) - Error handling and context management
- **`regex`** (v1.11.1) - Regular expressions (available for future use)

All dependencies are lightweight and commonly used in the Rust ecosystem.

## Testing

### Sample Files Included
The `input/` directory contains various BBL files for testing:
- **Betaflight logs**: `BTFL_*.BBL` files from different firmware versions
- **EmuFlight logs**: `EMUF_*.BBL` files for compatibility testing  
- **INAV logs**: `INAV_*.BBL` files in subdirectory
- **Different aircraft types**: Various flight controller boards and configurations

### Running Tests
```bash
# Test single file
./target/release/bbl_parser input/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.BBL

# Test multiple files with debug output
./target/release/bbl_parser --debug input/BTFL_*.BBL

# Test with timeout (recommended for large files)
timeout 60s ./target/release/bbl_parser input/*.BBL
```

## Development Roadmap

### Immediate Priorities
1. **Complete CSV Export**: Implement full CSV file writing functionality
2. **Advanced Statistics**: Add timing analysis, data rates, missing iteration calculations
3. **GPS Frame Support**: Complete G-frame field extraction and coordinate parsing
4. **Predictor System**: Full implementation of field prediction and delta reconstruction

### Future Enhancements
- **Performance Optimization**: Large file handling and memory efficiency improvements
- **Multi-Log Support**: Complete support for BBL files containing multiple flight logs
- **Additional Output Formats**: JSON export, GPX track generation for GPS data
- **Enhanced Error Handling**: Robust parsing with better error recovery
- **Real-time Processing**: Streaming log analysis capabilities

### Long-term Goals
- **Full JavaScript Parity**: Match all features of the reference implementation
- **Extended Format Support**: Support for newer BBL format variations
- **Integration Tools**: APIs for use in other Rust projects
- **Cross-platform Optimization**: Platform-specific performance enhancements

## Contributing

This parser aims to be a robust, feature-complete alternative to existing BBL parsers. Contributions are welcome, especially:

### High Priority Areas
- **Completing CSV export functionality**
- **Full GPS frame parsing and coordinate extraction**  
- **Predictor system implementation for accurate field reconstruction**
- **Advanced statistics matching JavaScript reference output**
- **Performance optimization for large file processing**

### Code Guidelines
- Follow the JavaScript reference implementation closely for compatibility
- Maintain clean separation between CLI (`main.rs`) and parsing logic (`bbl_format.rs`)
- Add comprehensive error handling for parsing edge cases
- Include test cases for new features using sample BBL files
- Document any deviations from the JavaScript reference

### Testing Requirements
- All changes must compile without errors: `cargo build --release`
- Test with multiple BBL file types (Betaflight, EmuFlight, INAV)
- Verify output matches expected format and statistics
- Performance testing with large files (use `timeout` for safety)

## License

This project is provided as-is for educational and practical use with Betaflight blackbox logs. 

## Acknowledgments

Based on the official JavaScript implementation from:
- [Betaflight Blackbox Log Viewer](https://github.com/betaflight/blackbox-log-viewer)
- Reference implementation maintainers and contributors
- Flight controller firmware developers (Betaflight, EmuFlight, INAV)

## Repository Information

- **Primary Module**: `src/main.rs` (main program)
- **Format Module**: `src/bbl_format.rs` (BBL format implementation) 
- **Configuration**: `Cargo.toml` (dependencies and build settings)
- **Documentation**: `Readme.md` (this file), `goals.md` (project objectives)
- **Test Data**: `input/` directory (sample BBL files for testing)
