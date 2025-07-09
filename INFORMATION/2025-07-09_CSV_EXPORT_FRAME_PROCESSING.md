# CSV Export Frame Processing - Improvements

**Date:** July 9, 2025

## Issues Addressed

1. **Step Response Rendering**
   - Step response plots were not being generated properly
   - Gyro renders had incorrect data visualization

2. **Frame Processing**
   - Frame filtering and ordering needed to match blackbox_decode exactly
   - S-frame data integration with I/P frames needed improvement

## Changes Implemented

1. **Improved Frame Filtering and Ordering**
   - Strict filtering to include only I, P, and S frames in CSV export
   - Process frame types in the correct order (I, P, S) to match blackbox_decode
   - S-frames are now properly merged but not directly written to CSV

2. **Better Timestamp Handling**
   - Use frame's actual timestamp for I and P frames when available
   - Fall back to calculated sequential timestamps only when needed
   - This preserves the original timing information from the flight log

3. **Improved S-Frame Data Integration**
   - S-frame data is now correctly merged into I and P frames
   - Matches blackbox_decode's exact lookup strategy:
     1. Try to find the value in the current frame
     2. If not found and it's a main frame (I/P), check latest S frame data
     3. Default to 0 if not found anywhere

4. **I-Frame S-Frame Merging Logic**
   - Only merge S-frame data that doesn't exist in I-frame
   - This follows blackbox_decode.c approach exactly
   - Prevents overwriting important frame-specific data

## Testing Results

The improved CSV export now correctly processes frames in the same way as blackbox_decode:

1. **Frame Processing**
   - Only I and P frames are included in the final CSV (S frames are merged)
   - Frames are processed in file order without timestamp-based sorting
   - S-frame data is correctly integrated with I and P frames

2. **Step Response and Gyro Rendering**
   - Step response rendering should now work with properly interleaved data
   - Gyro rendering is improved with correct data sequencing
   - Missing fields are properly handled with S-frame fallback

These changes ensure complete compatibility with blackbox_decode's CSV output format, which should solve the rendering issues with step responses and gyro data in the test scripts.

## Next Steps

1. **Testing with Various Flight Logs**
   - Test with diverse flight logs to ensure universal compatibility
   - Verify rendering works for all data types and frame sequences

2. **Performance Optimization**
   - Consider optimizing CSV export for large datasets
   - Profile for potential bottlenecks in frame processing
