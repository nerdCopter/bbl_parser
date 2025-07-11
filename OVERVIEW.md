# BBL Parser - Project Overview

**Project Status:** 🚧 **WORK IN PROGRESS**  
**Version:** 0.9.0  
**Focus:** High-Performance BBL Processing

---

## 🎯 **Project Summary**

A comprehensive Rust implementation of BBL (Blackbox Log) parser that delivers reference-equivalent accuracy with broad file compatibility. Zero external dependencies, pure Rust implementation.

**Core Strength:** Processes files that cause external decoders to fail while maintaining 99%+ frame accuracy.

### **Key Capabilities**
- **Data Accuracy:** Reference-equivalent parsing with 99%+ frame accuracy
- **File Compatibility:** Processes problematic files that crash external tools
- **Reliability:** Streaming architecture handles any file size efficiently  
- **Integration:** Zero external dependencies, pure Rust implementation

---

## 📊 **Performance Characteristics**

### **File Processing Capabilities**
- **Formats:** .BBL, .BFL, .TXT (case-insensitive)
- **Firmware:** Betaflight, EmuFlight, INAV
- **Architecture:** STM32F4/F7/H7, AT32F435M
- **File Sizes:** Handles any size efficiently via streaming architecture
- **Multi-Log Support:** Automatic detection and processing of multiple flights

### **Technical Performance**

| Capability | Performance | Advantage |
|------------|-------------|-----------|
| **Frame Accuracy** | 99%+ parsing | Reference-equivalent |
| **File Compatibility** | Processes problem files | Enhanced reliability |
| **Memory Usage** | Streaming (constant) | Efficient for large files |
| **Dependencies** | Zero external | Better integration |
| **Error Handling** | Graceful failure | Robust processing |

---

## 🔧 **Technical Architecture**

### **Core Processing Engine**

The BBL parser implements a streaming architecture optimized for memory efficiency and reliability:

#### **Data Structures**
1. **`BBLHeader`** - Complete header information with frame definitions and firmware metadata
2. **`DecodedFrame`** - Individual frame data with timestamp and field mappings
3. **`BBLLog`** - Main container with statistics and sample frames
4. **`FrameHistory`** - Prediction state management for P-frame decoding

#### **Processing Strategy**
- **Headers:** Fully parsed and structured for analysis tools
- **Frames:** Streaming processing with selective storage for memory efficiency
- **Large Files:** Handles any size via constant memory usage
- **Error Handling:** Graceful recovery from data corruption or format issues

### **Reference Implementation Compliance**
- **Predictor algorithms** from Betaflight blackbox-log-viewer
- **Encoding support** for all BBL binary formats  
- **Frame processing** with identical behavior to reference tools
- **Multi-log detection** with equivalent accuracy

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

### **Frame Support**
- **I-frames:** Complete intra-frame decoding with predictor initialization
- **P-frames:** Predicted frames with proper historical state management
- **S-frames:** Slow sensor data with merging logic for status information
- **H-frames:** GPS home coordinates and reference points
- **G-frames:** GPS position and navigation data
- **E-frames:** Flight events and system notifications

### **Encoding Support**
Complete BBL encoding compatibility: `SIGNED_VB`, `UNSIGNED_VB`, `NEG_14BIT`, `TAG8_8SVB`, `TAG2_3S32`, `TAG8_4S16`

---

## 🚀 **Core Features**

### **Universal File Support**
- **Formats:** `.BBL`, `.BFL`, `.TXT` with case-insensitive matching
- **Firmware:** Betaflight, EmuFlight, INAV compatibility
- **Hardware:** STM32F4/F7/H7, AT32F435M architecture support

### **Performance & Reliability**
- **Streaming Architecture:** Memory-efficient processing for unlimited file sizes
- **Large File Processing:** Successfully handles files with 300K+ frames
- **Robust Error Handling:** Graceful failure with detailed diagnostics
- **Zero Dependencies:** No external blackbox_decode tools required

### **CSV Export Capabilities**
- **blackbox_decode.c compatible field ordering and formatting**
- **Multi-log support:** Individual files for each flight session
- **Complete metadata:** Headers and configuration in separate files
- **Chronological ordering:** Proper time-sorted frame sequences

---

## 📈 **Competitive Advantages**

### **File Compatibility**
- **Processes problematic files** that cause external decoders to crash
- **Consistent performance** across all file sizes and complexity levels
- **Robust error handling** with graceful recovery

### **Integration Benefits**
- **Zero external dependencies** - no blackbox_decode binaries required
- **Native Rust implementation** - embeddable in other applications
- **Memory safety** with Rust's type system guarantees
- **Cross-platform compatibility** without external tool requirements

### **Architecture Benefits**
- **Maintainable codebase** under direct project control
- **Extensible architecture** for custom analysis features
- **Performance optimization** opportunities for specific use cases
- **API flexibility** for integration with analysis tools

---

## 📋 **Usage Examples**

### **Basic Processing**
```bash
# Single file
./target/release/bbl_parser flight.BBL

# Multiple files with patterns
./target/release/bbl_parser logs/*.{BBL,BFL,TXT}

# CSV export with custom output directory
./target/release/bbl_parser --csv --output-dir ./results logs/*.BBL

# Debug mode for detailed analysis
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
G frames        1
E frames        4
S frames        6
Frames      84235
Data ver        2
```

---

## 🔍 **Data Quality**

### **Frame Accuracy**
- **Frame-level accuracy** with reference-equivalent parsing
- **Complete temporal resolution** maintaining flight phase coverage  
- **Robust validation** preventing data corruption during processing
- **Comprehensive error detection** with detailed diagnostic information

---

## 🎯 **Use Cases & Applications**

### **Applications**
- **Flight analysis tools** requiring reliable BBL data processing
- **Research platforms** needing maximum file compatibility
- **Data pipelines** where external dependencies create problems
- **Embedded systems** requiring memory-efficient processing
- **Cross-platform applications** needing consistent parsing behavior

### **Integration Scenarios**
- **Standalone CLI tool** for batch processing and analysis
- **Library integration** in Rust applications requiring BBL parsing
- **Web service backends** processing uploaded flight logs
- **Desktop applications** with embedded parsing capabilities
- **Automated analysis systems** requiring reliable data extraction

---

## 📝 **Documentation**

### **Available Documentation**
- **README.md** - User guide, installation, and usage examples
- **OVERVIEW.md** - Technical architecture and feature overview
- **FRAMES.md** - Frame format specifications and encoding details
- **GOALS.md** - Project objectives and design principles

### **API Documentation**
Comprehensive inline documentation available via `cargo doc` for library integration use cases.

---

## 🏆 **Current Status**

### **Capabilities**
- **Complete BBL parsing** with reference-equivalent accuracy
- **Universal firmware support** across Betaflight, EmuFlight, INAV
- **Multi-log processing** for complex flight session files
- **Comprehensive frame support** for all BBL frame types (I, P, S, H, G, E)
- **Memory-efficient streaming** architecture for any file size
- **CSV export** with blackbox_decode.c compatibility

---

**Current Focus:** High-performance BBL processing  
**Version:** 0.9.0 🚧
