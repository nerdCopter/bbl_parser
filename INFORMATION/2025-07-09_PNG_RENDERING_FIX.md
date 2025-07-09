# PNG Rendering Improvement and Compatibility Fixes

**Date:** July 9, 2025

## Summary of Changes

We've restored the PNG rendering functionality by reverting some changes that were causing issues with the CSV export format. The following fixes were implemented:

### 1. Stream Handling Improvements

- Restored the original frame marker detection algorithm in `skip_to_next_marker`
- Added validation of potential markers by checking if the next byte can be read
- Reduced the search buffer size to 4096 bytes (from 8192) to match master's implementation
- Fixed false positive detection during frame parsing

### 2. Frame Selection Logic

- Simplified the frame selection for CSV export to match master's implementation
- Maintained the original way of including S-frame data
- Preserved the proper ordering of frames in the BBL file

### 3. S-frame Data Merging

- Restored the original S-frame data merging logic which only adds S-frame data if it doesn't already exist in I/P frames
- Simplified the S-frame collection and merging process
- Made sure S-frames are not directly written to CSV output

### 4. Timestamp Handling

- Reverted to the original timestamp handling which prefers the actual frame timestamp when available
- Fixed the timestamp formatting to match master's implementation
- Restored the correct type conversions for timestamps

### 5. Field Value Processing

- Fixed field value lookup and formatting to match master's implementation
- Restored the original way of handling special fields (energyCumulative, etc.)
- Made sure the formatted values match master's exact output format

These changes ensure that the CSV files produced by the parser are in the correct format for the PNG rendering tools, which results in properly rendered PNG files with correct data representation.

The gyro peaks and other visualizations should now appear correctly in the rendered PNG files, just like they did in the master branch.
