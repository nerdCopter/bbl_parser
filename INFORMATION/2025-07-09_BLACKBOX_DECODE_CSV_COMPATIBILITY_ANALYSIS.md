# Blackbox_decode CSV Export Compatibility Analysis

**Date:** July 9, 2025

## Summary of Findings

This document details how the blackbox_decode C implementation processes frames when generating CSV output, and how our Rust implementation needs to match it for complete compatibility.

## Frame Processing in blackbox_decode

After analyzing the C code in blackbox_decode.c, here's how it processes different frame types:

1. **I-frames (Intra frames):**
   - Written directly to CSV output
   - Contains complete state information
   - Uses actual timestamp from the frame
   - Merged with latest S-frame data if fields are missing

2. **P-frames (Predictive frames):**
   - Written directly to CSV output
   - Contains delta information based on previous frames
   - Uses actual timestamp from the frame
   - Merged with latest S-frame data if fields are missing

3. **S-frames (Slow frames):**
   - NOT written directly to CSV output (only stored in buffer)
   - Data is merged into subsequent I and P frames
   - Only S-frame fields not already present in I/P frames are added
   - In debug mode, S-frames are logged to CSV with a prefix

4. **Other frames (G, H, E):**
   - G (GPS) frames are processed separately and not included in main CSV
   - H (Home) frames are not included in CSV output
   - E (Event) frames are not included in CSV output

## Key Implementation Requirements

1. **Frame Selection:**
   - Only I and P frames should be written to the CSV
   - S frames should be used for merging data but not written directly
   - G, H, and E frames should be excluded from CSV output

2. **S-frame Data Merging:**
   - S-frame data should be stored in a buffer (latest_s_frame_data)
   - When processing I or P frames, fields from S frames should be merged ONLY if they don't already exist in the frame
   - This matches blackbox_decode's behavior of only adding S-frame fields not already present

3. **Timestamp Handling:**
   - Use the actual timestamp from the frame when available
   - Fall back to calculated timestamps only when necessary
   - This ensures proper synchronization with blackbox_decode's output

4. **Energy Calculation:**
   - Calculate energy cumulative based on timestamp deltas and current
   - Match blackbox_decode's rounding behavior for mAh values
   - Use the same time delta calculation (microseconds to hours)

5. **Output Formatting:**
   - Match blackbox_decode's spacing and number formatting
   - Ensure proper alignment of CSV columns
   - Use the same field names and order

## Implementation Status

Our Rust implementation now correctly:
- Filters frame types to match blackbox_decode
- Merges S-frame data with I and P frames using the same logic
- Handles timestamps and energy calculations in a compatible manner
- Formats output to match blackbox_decode's CSV layout

These changes ensure that our CSV output is fully compatible with the original blackbox_decode implementation, which should result in proper rendering of PNG charts and graphs.
