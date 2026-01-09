//! Export filtering heuristics for identifying logs worth exporting
//!
//! This module provides intelligent filtering functions to help identify flight logs
//! that should be skipped during export due to being ground tests, arm checks, or other
//! non-flight data.
//!
//! # Usage
//!
//! These filters are controlled via `ExportOptions`. CLI users get filtering enabled by
//! default for convenience, while library consumers can opt in/out as needed.

use crate::types::BBLLog;

/// Determines if a log should be skipped for export based on duration and frame count
///
/// Uses smart filtering: <5s always skip, 5-15s keep if good data density (>1500fps), >15s always keep
/// This helps eliminate ground tests, arm checks, and other non-flight activities.
///
/// # Arguments
/// * `log` - The BBL log to evaluate
/// * `force_export` - If true, never skips (overrides all heuristics)
///
/// # Returns
/// Tuple of (should_skip, reason_description)
pub fn should_skip_export(log: &BBLLog, force_export: bool) -> (bool, String) {
    if force_export {
        return (false, String::new()); // Never skip when forced
    }

    const VERY_SHORT_DURATION_MS: u64 = 5_000; // 5 seconds - always skip
    const SHORT_DURATION_MS: u64 = 15_000; // 15 seconds - threshold for normal logs
    const MIN_DATA_DENSITY_FPS: f64 = 1500.0; // Minimum fps for short logs
    const FALLBACK_MIN_FRAMES: u32 = 15_000; // ~10 seconds at 1500 fps (increased from 7500 to reduce false positives)

    // Check if we have duration information
    let duration_us = log.duration_us();
    if duration_us > 0 {
        // Use floating-point duration to avoid precision loss and division by zero
        let duration_s = duration_us as f64 / 1_000_000.0;

        // Guard against division by zero or very small durations
        if duration_s <= 0.0 {
            return (true, "duration too small or invalid".to_string());
        }

        let duration_ms = duration_us / 1000;
        let fps = log.stats.total_frames as f64 / duration_s;

        // Very short logs: < 5 seconds → Always skip
        if duration_ms < VERY_SHORT_DURATION_MS {
            return (true, format!("too short ({:.1}s < 5.0s)", duration_s));
        }

        // Short logs: 5-15 seconds → Keep if sufficient data density (>1500 fps)
        if duration_ms < SHORT_DURATION_MS {
            if fps < MIN_DATA_DENSITY_FPS {
                return (
                    true,
                    format!(
                        "insufficient data density ({:.0}fps < {:.0}fps for {:.1}s log)",
                        fps, MIN_DATA_DENSITY_FPS, duration_s
                    ),
                );
            }
            // Good data density, keep it
            return (false, String::new());
        }

        // Normal logs: > 15 seconds → Check for minimal gyro activity (ground tests)
        let (is_minimal_movement, max_range) = has_minimal_gyro_activity(log);
        if is_minimal_movement {
            return (
                true,
                format!(
                    "minimal gyro activity ({:.1} range) - likely ground test",
                    max_range
                ),
            );
        }

        return (false, String::new());
    }

    // No duration information available, fall back to frame count and gyro variance
    // Skip if very low frame count (equivalent to <10s at minimum viable fps)
    if log.stats.total_frames < FALLBACK_MIN_FRAMES {
        return (
            true,
            format!(
                "too few frames ({} < {}) and no duration info",
                log.stats.total_frames, FALLBACK_MIN_FRAMES
            ),
        );
    }

    // For logs without duration but sufficient frames, apply gyro range check
    // This catches INAV logs and older Betaflight logs that lack duration info
    let (is_minimal_movement, max_range) = has_minimal_gyro_activity(log);
    if is_minimal_movement {
        return (
            true,
            format!(
                "minimal gyro activity ({:.1} range) - likely ground test (no duration info)",
                max_range
            ),
        );
    }

    // Sufficient frames and meaningful gyro activity, keep it
    (false, String::new())
}

/// Analyzes gyro variance to detect ground tests vs actual flight
///
/// Uses range-normalized standard deviation to be scale-independent.
/// This works better than coefficient of variation when values cross zero.
///
/// Returns true if the log appears to be a static ground test (minimal movement)
///
/// # Arguments
/// * `log` - The BBL log to analyze
///
/// # Returns
/// Tuple of (is_minimal_movement, max_metric_value)
pub fn has_minimal_gyro_activity(log: &BBLLog) -> (bool, f64) {
    // Conservative thresholds to avoid false-skips
    const MIN_SAMPLES_FOR_ANALYSIS: usize = 15; // Reduced for limited sample data
    const MIN_GYRO_RANGE: f64 = 1500.0; // Minimum range to consider as actual flight activity (increased from 100)

    let mut gyro_x_values = Vec::new();
    let mut gyro_y_values = Vec::new();
    let mut gyro_z_values = Vec::new();

    // First try to use debug_frames if available (contains more comprehensive data)
    if let Some(debug_frames) = &log.debug_frames {
        // Collect gyro data from I and P frames in debug_frames
        for (frame_type, frames) in debug_frames {
            if *frame_type == 'I' || *frame_type == 'P' {
                for frame in frames {
                    if let Some(gyro_x) = frame.data.get("gyroADC[0]") {
                        if let Some(gyro_y) = frame.data.get("gyroADC[1]") {
                            if let Some(gyro_z) = frame.data.get("gyroADC[2]") {
                                gyro_x_values.push(*gyro_x as f64);
                                gyro_y_values.push(*gyro_y as f64);
                                gyro_z_values.push(*gyro_z as f64);
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback to frames if debug_frames not available or insufficient data
    if gyro_x_values.len() < MIN_SAMPLES_FOR_ANALYSIS {
        for frame in &log.frames {
            if let Some(gyro_x) = frame.data.get("gyroADC[0]") {
                if let Some(gyro_y) = frame.data.get("gyroADC[1]") {
                    if let Some(gyro_z) = frame.data.get("gyroADC[2]") {
                        gyro_x_values.push(*gyro_x as f64);
                        gyro_y_values.push(*gyro_y as f64);
                        gyro_z_values.push(*gyro_z as f64);
                    }
                }
            }
        }
    }

    // Need sufficient data points for reliable analysis
    if gyro_x_values.len() < MIN_SAMPLES_FOR_ANALYSIS {
        return (false, 0.0); // Not enough data, don't skip (conservative approach)
    }

    // Calculate range (max - min) for each axis
    // A ground test will have very small range regardless of gyro scale
    let range_x = calculate_range(&gyro_x_values);
    let range_y = calculate_range(&gyro_y_values);
    let range_z = calculate_range(&gyro_z_values);

    // Use the maximum range across all axes
    let max_range = range_x.max(range_y).max(range_z);

    // If all axes have minimal range, it's a ground test
    // Threshold of MIN_GYRO_RANGE (1500.0) provides clear separation - actual flights have ranges in thousands
    let is_minimal = max_range < MIN_GYRO_RANGE;

    (is_minimal, max_range)
}

/// Calculate range (max - min) of a dataset
///
/// # Arguments
/// * `values` - Slice of f64 values to compute range for
///
/// # Returns
/// The range of the dataset
pub fn calculate_range(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    max - min
}

/// Calculate variance of a dataset
///
/// DEPRECATED: This function is kept for backward compatibility (exported in public API)
/// but is no longer used by the filtering logic. The range-based detection approach
/// (see `calculate_range()`) is now preferred as it's scale-independent.
///
/// # Arguments
/// * `values` - Slice of f64 values to compute variance for
///
/// # Returns
/// The variance of the dataset
#[allow(dead_code)]
pub fn calculate_variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;

    variance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BBLLog, FrameStats};

    fn create_test_log(start_time_us: u64, end_time_us: u64, total_frames: u32) -> BBLLog {
        BBLLog {
            log_number: 1,
            total_logs: 1,
            header: Default::default(),
            stats: FrameStats {
                start_time_us,
                end_time_us,
                total_frames,
                ..Default::default()
            },
            frames: vec![],
            debug_frames: None,
            gps_coordinates: vec![],
            home_coordinates: vec![],
            event_frames: vec![],
        }
    }

    #[test]
    fn test_should_skip_very_short_flight() {
        // Less than 5 seconds should always skip
        let log = create_test_log(0, 3_000_000, 4500); // 3 seconds at 1500fps
        let (should_skip, reason) = should_skip_export(&log, false);
        assert!(should_skip, "Expected to skip flight shorter than 5s");
        assert!(reason.contains("too short"), "Expected 'too short' reason");
    }

    #[test]
    fn test_should_keep_five_seconds_with_good_density() {
        // 5 seconds at 1500fps should keep
        let log = create_test_log(0, 5_000_000, 7500); // 5 seconds at 1500fps
        let (should_skip, _) = should_skip_export(&log, false);
        assert!(
            !should_skip,
            "Expected to keep 5s flight with 1500fps density"
        );
    }

    #[test]
    fn test_should_skip_short_flight_low_density() {
        // 10 seconds but only 1000fps should skip
        let log = create_test_log(0, 10_000_000, 10_000); // 10 seconds at 1000fps
        let (should_skip, reason) = should_skip_export(&log, false);
        assert!(
            should_skip,
            "Expected to skip 10s flight with only 1000fps density"
        );
        assert!(
            reason.contains("insufficient data density"),
            "Expected 'insufficient data density' reason"
        );
    }

    #[test]
    fn test_force_export_overrides_skip() {
        // Even very short flight should not skip if force_export is true
        let log = create_test_log(0, 2_000_000, 3000); // 2 seconds
        let (should_skip, _) = should_skip_export(&log, true);
        assert!(!should_skip, "Expected force_export to prevent skip");
    }

    #[test]
    fn test_fallback_to_frame_count() {
        // No duration info, but sufficient frame count should keep (above 15,000 threshold)
        let log = create_test_log(0, 0, 16000); // 16000 frames, no duration
        let (should_skip, _) = should_skip_export(&log, false);
        assert!(
            !should_skip,
            "Expected to keep log with sufficient frame count"
        );
    }

    #[test]
    fn test_fallback_to_frame_count_too_low() {
        // No duration info, insufficient frame count should skip
        let log = create_test_log(0, 0, 10000); // 10000 frames, no duration (below 15000 threshold)
        let (should_skip, reason) = should_skip_export(&log, false);
        assert!(should_skip, "Expected to skip log with too few frames");
        assert!(
            reason.contains("too few frames"),
            "Expected 'too few frames' reason"
        );
    }
}
