## This project reads data into internal data structures

The BBL parser project **does read data into internal data structures**, but it uses a **streaming approach with selective storage** to manage memory efficiently. Here's how it works:

### Data Structures Used:

1. **`BBLHeader`** - Stores all header information including:
   - Firmware info, board info, craft name
   - Frame definitions for I, P, S, G, H frames
   - System configuration
   - All header lines

2. **`DecodedFrame`** - Stores individual frame data:
   - Frame type, timestamp, loop iteration
   - HashMap of field names to values

3. **`BBLLog`** - Main container holding:
   - Header information
   - Frame statistics 
   - Sample frames (limited set)
   - Debug frames (when enabled)

4. **`FrameHistory`** - Maintains prediction state:
   - Current, previous, and previous2 frame data
   - Used for P-frame decoding which relies on prediction

### Storage Strategy:

The parser uses a **selective storage approach**:

- **Headers**: Fully parsed and stored in structured format
- **Frames**: 
  - Parses all frames but only stores **sample frames** (not all frames)
  - Stores first few frames, some middle frames, and last frames
  - In debug mode, stores up to 50 frames per frame type for analysis
  - Maintains frame history for prediction during parsing

### Key Evidence from Code:

```rust
struct BBLLog {
    log_number: usize,
    total_logs: usize,
    header: BBLHeader,
    stats: FrameStats,
    sample_frames: Vec<DecodedFrame>, // Only store a few sample frames, not all
    debug_frames: Option<HashMap<char, Vec<DecodedFrame>>>, // Frame data by type for debug output
}
```

The comment explicitly states "Only store a few sample frames, not all", showing it's designed to read all data but selectively store only what's needed for analysis.

### Purpose:

This design allows the parser to:
- Fully decode and validate all frame data
- Generate comprehensive statistics
- Provide sample data for analysis
- Support CSV export capabilities
- Maintain reasonable memory usage even with large BBL files

**The parser reads data into internal data structures, but uses a streaming approach with selective storage rather than loading everything into memory.**

