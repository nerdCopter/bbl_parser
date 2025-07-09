# Frame Parsing Error Recovery Analysis

**Date:** July 9, 2025

## Problem Summary

Despite implementing blackbox_decode CSV compatibility fixes, the RUST parser is still producing excessive "Failed frames" counts (28K+ errors) compared to blackbox_decode which shows minimal failures (0-2 failed frames).

## Key Issues Identified

1. **Stream Recovery Logic**: Our current error recovery using `skip_to_next_marker()` may be too aggressive or getting stuck in loops.

2. **Failed Frame Counting**: We're incrementing `failed_frames` too liberally. Blackbox_decode is much more conservative about what constitutes a "failed" frame.

3. **Safety Limits**: The parser hits safety limits (500K failed frames) which prevents complete parsing of logs.

## Changes Made

### Stream Recovery Improvements
- Updated `skip_to_next_marker()` to be more permissive (increased buffer from 4096 to 8192 bytes)
- Made frame marker detection more optimistic, similar to blackbox_decode's approach
- Removed extra validation checks that could cause false positives

### Error Handling Improvements
- Added proper else branches for I-frame, S-frame, and other frame parsing failures
- Replaced aggressive `skip_frame()` calls with gentler `skip_to_next_marker()` recovery
- Only increment `failed_frames` when recovery is truly impossible

### Frame Processing Logic
- Ensured all frame types (I, P, S, H, G, E) have proper error recovery
- Removed the unused `skip_frame()` function which was too strict
- Added position tracking to detect when stream is not advancing

## Remaining Issues

The failed frame count is still too high, suggesting:
1. The fundamental frame parsing algorithm may need adjustment
2. Stream corruption detection might be too sensitive
3. The encoding/decoding logic might have bugs causing parse failures

## Next Steps

1. Compare frame-by-frame parsing with blackbox_decode to identify differences
2. Add more detailed logging to understand where parsing failures occur
3. Consider implementing blackbox_decode's exact error recovery strategy
4. Investigate if the frame size calculation or stream positioning is incorrect

## Impact on CSV/PNG Output

While the CSV output format has been fixed for compatibility, the high number of parsing errors suggests we're missing significant amounts of flight data that blackbox_decode successfully parses.
