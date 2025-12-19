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
    const FALLBACK_MIN_FRAMES: u32 = 7_500; // ~5 seconds at 1500 fps (fallback when no duration)

    // Check if we have duration information
    if log.stats.start_time_us > 0 && log.stats.end_time_us > log.stats.start_time_us {
        let duration_us = log
            .stats
            .end_time_us
            .saturating_sub(log.stats.start_time_us);
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
        if duration_ms >= SHORT_DURATION_MS {
            let (is_minimal_movement, max_variance) = has_minimal_gyro_activity(log);
            if is_minimal_movement {
                return (
                    true,
                    format!(
                        "minimal gyro activity ({:.1} variance) - likely ground test",
                        max_variance
                    ),
                );
            }
        }

        return (false, String::new());
    }

    // No duration information available, fall back to frame count
    // Skip if very low frame count (equivalent to <5s at minimum viable fps)
    if log.stats.total_frames < FALLBACK_MIN_FRAMES {
        return (
            true,
            format!(
                "too few frames ({} < {}) and no duration info",
                log.stats.total_frames, FALLBACK_MIN_FRAMES
            ),
        );
    }

    // Sufficient frames without duration info, keep it
    (false, String::new())
}

/// Analyzes gyro variance to detect ground tests vs actual flight
///
/// Returns true if the log appears to be a static ground test (minimal movement)
///
/// # Arguments
/// * `log` - The BBL log to analyze
///
/// # Returns
/// Tuple of (is_minimal_movement, max_variance_value)
pub fn has_minimal_gyro_activity(log: &BBLLog) -> (bool, f64) {
    // Conservative thresholds to avoid false-skips
    const MIN_SAMPLES_FOR_ANALYSIS: usize = 15; // Reduced for limited sample data
    const VERY_LOW_GYRO_VARIANCE_THRESHOLD: f64 = 0.3; // More aggressive threshold for ground test detection

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

    // Calculate variance for each axis
    let variance_x = calculate_variance(&gyro_x_values);
    let variance_y = calculate_variance(&gyro_y_values);
    let variance_z = calculate_variance(&gyro_z_values);

    // Use the maximum variance across all axes
    let max_variance = variance_x.max(variance_y).max(variance_z);

    // Very conservative: only skip if ALL axes show extremely low variance
    let is_minimal = max_variance < VERY_LOW_GYRO_VARIANCE_THRESHOLD;

    (is_minimal, max_variance)
}

/// Calculate variance of a dataset
///
/// # Arguments
/// * `values` - Slice of f64 values to compute variance for
///
/// # Returns
/// The variance of the dataset
pub fn calculate_variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;

    variance
}
