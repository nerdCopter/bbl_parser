# BBL Parser v0.9.0 - Project Overview

**Project Status:** 🎯 **PHASE 1 COMPLETE - Rust Crate Structure**  
**Version:** 0.9.0  
**Focus:** Library + CLI Architecture  
**📦 Ready for Library Usage**

---

## 🎯 **Recent Major Achievement: Phase 1 Complete**

**August 9, 2025 - Rust Crate Architecture Implemented**

✅ **Successfully refactored into proper Rust crate structure:**
- **Library**: `src/lib.rs` with public API for external usage
- **Binary**: `src/bin/main.rs` for CLI functionality
- **Features**: Modular feature-based configuration
- **API**: Memory-efficient data access methods implemented
- **Quality**: All 11 tests passing, clippy/fmt compliance maintained

✅ **Public API now available for external crate usage:**
```rust
use bbl_parser::{parse_bbl_file, ExportOptions, BBLLog};

let log = parse_bbl_file(Path::new("flight.BBL"), ExportOptions::default(), false)?;
let gyro_data = log.get_gyro_data();
let pid_data = log.get_pid_data();
```

✅ **CLI functionality 100% preserved - no breaking changes for existing users**

**Next**: Phase 2 will move actual parsing logic from main.rs to library implementation.

---

## 🎯 **Project Summary**

A comprehensive Rust implementation of BBL (Blackbox Log) parser designed for flight controller blackbox data analysis. **Now structured as a proper Rust crate suitable for both library and CLI usage.**

**Core Goal:** Create a reliable BBL parser that can handle various file formats and firmware types while maintaining memory efficiency and providing both library and CLI interfaces.

### **Current Capabilities**
- **🆕 Library API:** Public API for external crate integration with memory-efficient data access
- **🆕 CLI Preserved:** All existing command-line functionality maintained
- **BBL Format Support:** Parses .BBL, .BFL, .TXT files from multiple firmware sources
- **Frame Processing:** Supports I, P, S, H, G, E frames with proper encoding handling
- **Export Functions:** CSV, GPX, and event data export capabilities
- **Memory Efficiency:** Streaming architecture for large file processing
- **Zero Dependencies:** Pure Rust implementation without external blackbox_decode tools

---

## 📊 **Current Implementation Status**

### **File Processing Capabilities**
- **Formats:** .BBL, .BFL, .TXT (case-insensitive)
- **Firmware:** Betaflight, EmuFlight, INAV
- **File Sizes:** Handles large files via streaming architecture (tested up to 240K+ rows)
- **Multi-Log Support:** Automatic detection and processing of multiple flights

### **Development Status**

| Feature | Status | Implementation |
|---------|--------|----------------|
| **Basic Parsing** | ✅ Functional | Core frame parsing implemented |
| **CSV Export** | ✅ Functional | blackbox_decode compatible format |
| **GPS Export** | ✅ Functional | GPX format generation |
| **Event Export** | ✅ Functional | JSONL format with Betaflight event types |
| **Multi-log Processing** | ✅ Functional | Automatic detection |
| **Error Handling** | 🚧 Basic | Needs comprehensive testing |
| **Performance** | 🚧 Basic | Optimization in progress |
| **Testing** | ⚠️ Limited | Needs extensive validation |

---

## 🔧 **Technical Architecture**

### **Core Processing Engine**

The BBL parser implements a streaming architecture designed for memory efficiency:

#### **Data Structures**
1. **`BBLHeader`** - Complete header information with frame definitions and firmware metadata
2. **`DecodedFrame`** - Individual frame data with timestamp and field mappings
3. **`BBLLog`** - Main container with statistics and sample frames
4. **`FrameHistory`** - Prediction state management for P-frame decoding
5. **`GpsCoordinate`** - GPS position data with coordinate conversion
6. **`EventFrame`** - Flight event data with Betaflight event type mapping

#### **Processing Strategy**
- **Headers:** Fully parsed and structured for analysis tools
- **Frames:** Streaming processing with selective storage for memory efficiency
- **Large Files:** Handles large files via streaming with controlled memory usage
- **Error Handling:** Basic error recovery with diagnostic output

### **Frame Support Implementation**
- **I-frames:** Complete intra-frame decoding with predictor initialization
- **P-frames:** Predicted frames with historical state management
- **S-frames:** Slow sensor data with merging logic for status information
- **H-frames:** GPS home coordinates and reference points
- **G-frames:** GPS position and navigation data with differential encoding
- **E-frames:** Flight events with official Betaflight FlightLogEvent enum mapping

### **Export Functionality**
- **CSV Export:** blackbox_decode compatible format with proper field ordering
- **GPX Export:** Standard GPS exchange format for mapping applications  
- **Event Export:** JSONL format with Betaflight event type descriptions

### **Encoding Support**
BBL encoding compatibility: `SIGNED_VB`, `UNSIGNED_VB`, `NEG_14BIT`, `TAG8_8SVB`, `TAG2_3S32`, `TAG8_4S16`

### **Project Structure**
```
src/
├── main.rs              # CLI interface, file handling, statistics
├── bbl_format.rs        # BBL binary format decoding and encoding
├── error.rs             # Error handling and result types
├── types/               # Core data structures
│   ├── mod.rs          #   Module definitions
│   ├── log.rs          #   Log container types
│   ├── frame.rs        #   Frame data structures
│   └── header.rs       #   Header information types
└── parser/              # Parsing implementation
    ├── mod.rs          #   Parser module definitions
    ├── decoder.rs      #   Frame decoding logic
    ├── frame.rs        #   Frame parsing implementations
    ├── header.rs       #   Header parsing logic
    └── stream.rs       #   Binary stream handling
```

---

## 🚀 **Current Features**

### **File Processing**
- **Universal Format Support:** `.BBL`, `.BFL`, `.TXT` with case-insensitive matching
- **Firmware Compatibility:** Betaflight, EmuFlight, INAV support
- **Multi-log Processing:** Automatic detection of multiple flight sessions in single files

### **Data Export Capabilities**
- **CSV Export:** blackbox_decode compatible field ordering and formatting
- **GPS Export:** GPX format generation for mapping applications
- **Event Export:** Flight event data in JSONL format with Betaflight event type mapping
- **Multi-log Support:** Individual files for each flight session
- **Metadata Export:** Complete header and configuration information

### **Architecture Benefits**
- **Memory Efficiency:** Streaming architecture for large file processing
- **Zero External Dependencies:** No blackbox_decode binaries required
- **Native Rust Implementation:** Embeddable in other applications
- **Cross-platform Compatibility:** Works without external tool requirements

---

## ⚠️ **Development Limitations**

### **Current Limitations**
- **Testing Coverage:** Limited testing across all firmware versions and file formats
- **Performance:** Not fully optimized for all use cases
- **Error Handling:** Basic error recovery, may not handle all edge cases gracefully
- **API Stability:** Implementation may change between versions
- **Documentation:** Limited API documentation for library use

### **Known Issues**
- **File Compatibility:** May not handle all edge cases that external decoders encounter
- **Performance:** Not optimized for extremely large files (>1M frames)
- **Validation:** Limited validation against reference implementations
- **Error Messages:** May not provide detailed diagnostic information for all failures

---

## 📈 **Development Focus Areas**

### **Current Priorities**
- **Testing:** Comprehensive validation across firmware versions
- **Performance:** Optimization for large file processing
- **Error Handling:** Improved diagnostic and recovery capabilities
- **Documentation:** Complete API documentation and usage examples
- **Compatibility:** Enhanced support for edge cases and unusual file formats

---

## 🔍 **Current Data Processing**

### **Frame Processing**
- **Frame Parsing:** Basic parsing of all major frame types
- **Temporal Resolution:** Maintains flight sequence timing
- **Error Detection:** Basic validation with diagnostic output
- **Memory Management:** Streaming architecture for large files

### **Export Quality**
- **CSV Compatibility:** Basic blackbox_decode format compatibility
- **GPS Accuracy:** Coordinate conversion with firmware-specific scaling
- **Event Mapping:** Official Betaflight FlightLogEvent enum compliance

---

## 📈 **Development Focus Areas**

### **Current Priorities**
- **Testing:** Comprehensive validation across firmware versions
- **Performance:** Optimization for large file processing
- **Error Handling:** Improved diagnostic and recovery capabilities
- **Documentation:** Complete API documentation and usage examples
- **Compatibility:** Enhanced support for edge cases and unusual file formats

---

## 📋 **Usage Examples**

### **Basic Processing**
```bash
# Single file analysis
./target/release/bbl_parser flight.BBL

# Multiple files with patterns
./target/release/bbl_parser logs/*.{BBL,BFL,TXT}

# CSV export with custom output directory
./target/release/bbl_parser --csv --output-dir ./results logs/*.BBL

# GPS data export to GPX format
./target/release/bbl_parser --gpx flight_with_gps.BBL

# Event data export
./target/release/bbl_parser --event logs/*.BBL

# All export formats
./target/release/bbl_parser --csv --gpx --event logs/*.BBL

# Debug mode for development analysis
./target/release/bbl_parser --debug problematic_file.BBL
```

### **Typical Output**
```
Processing: BTFL_BLACKBOX_LOG_20250601_121852.BBL

Log 1 of 3, frames: 84235
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

Exported GPS data to: BTFL_BLACKBOX_LOG_20250601_121852.gps.gpx
Exported event data to: BTFL_BLACKBOX_LOG_20250601_121852.event
```

---

## 🔍 **Current Data Processing**

### **Frame Processing**
- **Frame Parsing:** Basic parsing of all major frame types
- **Temporal Resolution:** Maintains flight sequence timing
- **Error Detection:** Basic validation with diagnostic output
- **Memory Management:** Streaming architecture for large files

### **Export Quality**
- **CSV Compatibility:** Basic blackbox_decode format compatibility
- **GPS Accuracy:** Coordinate conversion with firmware-specific scaling
- **Event Mapping:** Official Betaflight FlightLogEvent enum compliance

---

## 🎯 **Use Cases & Applications**

### **Current Applications**
- **Flight log analysis** for debugging and performance tuning
- **Data extraction** from BBL files for further processing
- **Format conversion** from BBL to CSV/GPX formats
- **Development and testing** of BBL parsing algorithms
- **Educational use** for understanding blackbox log structures

### **Development Integration**
- **Standalone CLI tool** for batch processing and analysis
- **Rust library integration** (with API stability caveats)
- **Custom analysis applications** requiring BBL parsing capabilities
- **Research and development** of flight data analysis techniques

---

## 📝 **Documentation**

### **Available Documentation**
- **README.md** - User guide, installation, and usage examples
- **OVERVIEW.md** - Technical architecture and feature overview  
- **FRAMES.md** - Frame format specifications and encoding details
- **GOALS.md** - Project objectives and design principles

### **Development Documentation**
Limited API documentation available via `cargo doc` for development use.

---

## 🏆 **Current Status Summary**

### **Functional Capabilities**
- **Basic BBL parsing** with support for major frame types
- **Multi-firmware support** across Betaflight, EmuFlight, INAV
- **Multi-log processing** for complex flight session files
- **Export functionality** for CSV, GPX, and event data formats
- **Memory-efficient streaming** architecture for large files

### **Development Status**
- **Core functionality** implemented and functional
- **Testing coverage** limited and needs expansion
- **Performance** adequate but not optimized
- **Error handling** basic with room for improvement
- **API stability** not guaranteed between versions

---

**Current Focus:** Core functionality development and testing  
**Version:** 0.9.0 🚧 Work-in-Progress  
**⚠️ Not recommended for production use**
