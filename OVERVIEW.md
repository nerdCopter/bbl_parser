# BBL Parser - Project Overview

**Project Status:** 🔧 **WORK IN PROGRESS - TIMING ISSUES REMAIN**  
**Version:** 0.9 (Work in Progress, Not Production Ready)  
**Last Updated:** July 4, 2025

---

## 🎯 **Project Summary**

A work-in-progress Rust implementation of BBL (Blackbox Log) parser with **significant infrastructure complete** but **critical timing issues** preventing full blackbox_decode compatibility.

**CURRENT STATUS (July 4, 2025):**
- ✅ **Infrastructure Progress**: Major blackbox_decode.c methodology implemented
- ✅ **Some Data Fields**: Voltage, motor, accelerometer scaling appears correct  
- ✅ **Frame Structure**: Basic I/P/S frame parsing implemented
- ❌ **CRITICAL TIMING BUG**: P-frame time field extracts wrong raw deltas (-6,1,0 vs ~304)
- ❌ **COMPATIBILITY INCOMPLETE**: Does not fully match blackbox_decode.c output
- ❌ **FIELD ORDERING**: CSV column ordering differs from reference

**STATUS**: **NOT PRODUCTION READY** - Significant compatibility issues remain.

**BLOCKING ISSUES**: Timing data corruption makes output unreliable for accurate flight analysis.

**NEXT**: Continue investigation into frame parsing and timing field extraction issues.

---

## 🏗️ **Architecture Overview**

### **Core Components**
- **BBL Decoder (`bbl_format.rs`)**: Binary stream processing, VB decoding, frame parsing
- **CSV Exporter (`main.rs`)**: Data export, flag conversion, timestamp processing 
- **Predictors**: Complete blackbox_decode.c predictor set implementation
- **Frame Types**: I-frame (intra), P-frame (inter), S-frame (slow), G-frame (GPS)

### **Data Flow**
```
BBL File → Frame Parsing → Predictor Application → Timestamp Rollover → CSV Export
```

### **Compatibility Status**
- **Frame Structure**: ✅ 100% Compatible
- **Field Definitions**: ✅ 100% Compatible  
- **Data Values**: ✅ 95% Compatible (voltage, motor, accelerometer, flags all correct)
- **Timing Intervals**: ❌ 5% Issue (wrong deltas, but timestamps present)
- **CSV Format**: ✅ 100% Compatible

---

## 📊 **Current Development Status**

### **What's Working**
- **Basic Frame Parsing**: I/P/S frame structure recognition
- **Some Data Fields**: Voltage and motor values appear to match scale
- **Infrastructure**: Basic predictor and VB decoding framework
- **Memory Usage**: Efficient streaming processing

### **What's Not Working**
- **❌ Timing Data**: Wrong intervals vs blackbox_decode (critical issue)
- **❌ Field Ordering**: CSV column differences vs reference
- **❌ Flight Mode Flags**: Value extraction issues
- **❌ Full Compatibility**: Does not match blackbox_decode.c output

---

## 🔍 **Critical Issues**

### **1. Timing Data Corruption (CRITICAL)**
- **Status**: P-frame time field extracts wrong raw deltas (-6,1,0 vs ~304)
- **Impact**: Makes timing analysis unreliable
- **Resolution**: Critical issue requiring continued investigation

### **2. Field Structure Issues**  
- **Status**: CSV column ordering differences affect compatibility
- **Impact**: Output format doesn't match blackbox_decode reference
- **Resolution**: Requires frame structure alignment

### **3. Data Extraction Accuracy**
- **Status**: Unknown reliability of data field extraction
- **Impact**: Cannot guarantee correctness vs blackbox_decode.c
- **Resolution**: Needs comprehensive validation

---

## 🎯 **Project Status Assessment**

### **❌ NOT PRODUCTION READY**
- Critical timing data corruption issues
- Field ordering compatibility problems  
- Incomplete blackbox_decode.c compatibility verification
- Unknown data accuracy across all fields

### **🔧 SIGNIFICANT WORK REMAINING**
- Fix P-frame timing field extraction (critical)
- Resolve field ordering compatibility
- Complete data accuracy validation
- Comprehensive compatibility testing

### **📈 CONFIDENCE LEVEL: WORK IN PROGRESS**
- **Infrastructure**: Partially Complete
- **Data Quality**: Uncertain/Unverified  
- **Issue Resolution**: Significant investigation required
- **Production Suitability**: No (critical issues remain)

**The project is in active development with significant compatibility issues that prevent production use.**
