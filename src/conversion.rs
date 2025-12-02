//! Data conversion utilities for BBL parsing
//!
//! Contains all firmware-aware conversion functions for voltage, GPS data,
//! and flag formatting to maintain compatibility across firmware versions.

use semver::Version;

/// Convert raw vbat value to volts with firmware-aware scaling
pub fn convert_vbat_to_volts(raw_value: i32, firmware_revision: &str) -> f32 {
    // Determine scaling factor based on firmware
    let scale_factor = if firmware_revision.contains("EmuFlight") {
        // EmuFlight always uses tenths
        0.1
    } else if firmware_revision.contains("iNav") {
        // iNav always uses hundredths
        0.01
    } else if firmware_revision.contains("Betaflight") {
        // Betaflight version-dependent scaling
        if let Some(version) = extract_firmware_version(firmware_revision) {
            if version >= Version::new(4, 3, 0) {
                0.01 // hundredths for >= 4.3.0
            } else {
                0.1 // tenths for < 4.3.0
            }
        } else {
            // Default to modern Betaflight scaling if version can't be parsed
            0.01
        }
    } else {
        // Unknown firmware, default to hundredths
        0.01
    };

    raw_value as f32 * scale_factor
}

/// Extract version from firmware revision string
pub fn extract_firmware_version(firmware_revision: &str) -> Option<Version> {
    // Parse version from strings like "Betaflight 4.5.1 (77d01ba3b) AT32F435M"
    let words: Vec<&str> = firmware_revision.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        if word.to_lowercase().contains("betaflight") && i + 1 < words.len() {
            if let Ok(version) = Version::parse(words[i + 1]) {
                return Some(version);
            }
        }
    }
    None
}

/// Converts raw amperageLatest value to amps (0.01A units)
pub fn convert_amperage_to_amps(raw_value: i32) -> f32 {
    raw_value as f32 / 100.0
}

/// Extract major firmware version number
pub fn extract_major_firmware_version(firmware_revision: &str) -> u8 {
    // Extract major version from firmware string like "Betaflight 4.5.1 (77d01ba3b) AT32F435M"
    if let Some(start) = firmware_revision.find(' ') {
        let version_part = &firmware_revision[start + 1..];
        if let Some(end) = version_part.find('.') {
            if let Ok(major) = version_part[..end].parse::<u8>() {
                return major;
            }
        }
    }
    // Default to 4 if parsing fails (assume modern firmware)
    4
}

/// Convert GPS coordinate from raw value to degrees
pub fn convert_gps_coordinate(raw_value: i32) -> f64 {
    // GPS coordinates are stored as degrees * 10000000
    raw_value as f64 / 10_000_000.0
}

/// Convert GPS altitude with firmware-aware unit conversion
pub fn convert_gps_altitude(raw_value: i32, firmware_revision: &str) -> f64 {
    // Altitude units changed between firmware versions:
    // Before Betaflight 4: centimeters (factor 0.01)
    // Betaflight 4+: decimeters (factor 0.1)
    let major_version = extract_major_firmware_version(firmware_revision);
    if major_version >= 4 {
        raw_value as f64 / 10.0 // decimeters to meters
    } else {
        raw_value as f64 / 100.0 // centimeters to meters
    }
}

/// Convert GPS speed from raw value to m/s
pub fn convert_gps_speed(raw_value: i32) -> f64 {
    // Speed is stored as cm/s * 100, convert to m/s
    raw_value as f64 / 100.0
}

/// Convert GPS course from raw value to degrees
pub fn convert_gps_course(raw_value: i32) -> f64 {
    // Course is stored as degrees * 10
    raw_value as f64 / 10.0
}

/// Format flight mode flags for CSV output
pub fn format_flight_mode_flags(flags: i32) -> String {
    let mut modes = Vec::new();

    // Based on Betaflight firmware runtime_config.h flightModeFlags_e enum
    // This matches the blackbox-tools implementation exactly:
    // https://github.com/betaflight/blackbox-tools/blob/master/src/blackbox_fielddefs.c

    // FLIGHT_LOG_FLIGHT_MODE_NAME array from blackbox-tools
    if (flags & (1 << 0)) != 0 {
        modes.push("ANGLE_MODE"); // ANGLE_MODE = (1 << 0)
    }
    if (flags & (1 << 1)) != 0 {
        modes.push("HORIZON_MODE"); // HORIZON_MODE = (1 << 1)
    }
    if (flags & (1 << 2)) != 0 {
        modes.push("MAG"); // MAG_MODE = (1 << 2)
    }
    if (flags & (1 << 3)) != 0 {
        modes.push("BARO"); // ALT_HOLD_MODE = (1 << 3) (old name BARO)
    }
    if (flags & (1 << 4)) != 0 {
        modes.push("GPS_HOME"); // GPS_HOME_MODE (disabled in current firmware)
    }
    if (flags & (1 << 5)) != 0 {
        modes.push("GPS_HOLD"); // POS_HOLD_MODE = (1 << 5) (old name GPS_HOLD)
    }
    if (flags & (1 << 6)) != 0 {
        modes.push("HEADFREE"); // HEADFREE_MODE = (1 << 6)
    }
    if (flags & (1 << 7)) != 0 {
        modes.push("UNUSED"); // CHIRP_MODE = (1 << 7) (old autotune, now unused)
    }
    if (flags & (1 << 8)) != 0 {
        modes.push("PASSTHRU"); // PASSTHRU_MODE = (1 << 8)
    }
    if (flags & (1 << 9)) != 0 {
        modes.push("RANGEFINDER_MODE"); // RANGEFINDER_MODE (disabled in current firmware)
    }
    if (flags & (1 << 10)) != 0 {
        modes.push("FAILSAFE_MODE"); // FAILSAFE_MODE = (1 << 10)
    }
    if (flags & (1 << 11)) != 0 {
        modes.push("GPS_RESCUE_MODE"); // GPS_RESCUE_MODE = (1 << 11) (new in current firmware)
    }

    if modes.is_empty() {
        "0".to_string()
    } else {
        modes.join("|") // Use pipe separator to avoid breaking CSV format
    }
}

/// Format state flags for CSV output
pub fn format_state_flags(flags: i32) -> String {
    let mut states = Vec::new();

    // Based on Betaflight firmware runtime_config.h stateFlags_t enum
    // This matches the blackbox-tools implementation exactly:
    // https://github.com/betaflight/blackbox-tools/blob/master/src/blackbox_fielddefs.c

    // FLIGHT_LOG_FLIGHT_STATE_NAME array from blackbox-tools
    if (flags & (1 << 0)) != 0 {
        states.push("GPS_FIX_HOME"); // GPS_FIX_HOME = (1 << 0)
    }
    if (flags & (1 << 1)) != 0 {
        states.push("GPS_FIX"); // GPS_FIX = (1 << 1)
    }
    if (flags & (1 << 2)) != 0 {
        states.push("CALIBRATE_MAG"); // GPS_FIX_EVER = (1 << 2) but old name CALIBRATE_MAG
    }
    if (flags & (1 << 3)) != 0 {
        states.push("SMALL_ANGLE"); // Used in blackbox-tools for compatibility
    }
    if (flags & (1 << 4)) != 0 {
        states.push("FIXED_WING"); // Used in blackbox-tools for compatibility
    }

    if states.is_empty() {
        "0".to_string()
    } else {
        states.join("|") // Use pipe separator to avoid breaking CSV format
    }
}

/// Format failsafe phase for CSV output
pub fn format_failsafe_phase(phase: i32) -> String {
    // Based on Betaflight firmware failsafe.h failsafePhase_e enum
    // This matches the blackbox-tools implementation exactly:
    // https://github.com/betaflight/blackbox-tools/blob/master/src/blackbox_fielddefs.c

    // FLIGHT_LOG_FAILSAFE_PHASE_NAME array from blackbox-tools
    match phase {
        0 => "IDLE".to_string(),               // FAILSAFE_IDLE = 0
        1 => "RX_LOSS_DETECTED".to_string(),   // FAILSAFE_RX_LOSS_DETECTED
        2 => "LANDING".to_string(),            // FAILSAFE_LANDING
        3 => "LANDED".to_string(),             // FAILSAFE_LANDED
        4 => "RX_LOSS_MONITORING".to_string(), // FAILSAFE_RX_LOSS_MONITORING (new in current firmware)
        5 => "RX_LOSS_RECOVERED".to_string(), // FAILSAFE_RX_LOSS_RECOVERED (new in current firmware)
        6 => "GPS_RESCUE".to_string(),        // FAILSAFE_GPS_RESCUE (new in current firmware)
        _ => phase.to_string(),
    }
}

// ============================================================================
// GPX Timestamp Generation (for GPS export)
// ============================================================================

/// Generate GPX timestamp from log_start_datetime header + frame timestamp.
/// Following blackbox_decode approach: dateTime + (gpsFrameTime / 1000000)
/// If log_start_datetime is not available or invalid, falls back to relative time from epoch.
pub fn generate_gpx_timestamp(log_start_datetime: Option<&str>, frame_timestamp_us: u64) -> String {
    let total_seconds = frame_timestamp_us / 1_000_000;
    let microseconds = frame_timestamp_us % 1_000_000;

    // Try to parse the log start datetime if available
    if let Some(datetime_str) = log_start_datetime {
        // Check for placeholder datetime (clock not set on FC)
        if datetime_str.starts_with("0000-01-01") {
            // FC clock wasn't set, fall back to relative time
            return format_relative_timestamp(total_seconds, microseconds);
        }

        // Parse ISO 8601 datetime: "2024-10-10T18:37:25.559+00:00"
        // We only need the date and base time parts for combining with frame offset
        if let Some(base_time) = parse_datetime_to_epoch(datetime_str) {
            let absolute_secs = base_time + total_seconds;

            // Convert back to date/time components
            let secs_per_minute = 60u64;
            let secs_per_hour = 3600u64;
            let secs_per_day = 86400u64;

            // Calculate time components
            let time_of_day = absolute_secs % secs_per_day;
            let hours = (time_of_day / secs_per_hour) % 24;
            let minutes = (time_of_day % secs_per_hour) / secs_per_minute;
            let seconds = time_of_day % secs_per_minute;

            // Calculate date components (days since epoch 1970-01-01)
            let days_since_epoch = absolute_secs / secs_per_day;
            let (year, month, day) = days_to_ymd(days_since_epoch);

            return format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
                year, month, day, hours, minutes, seconds, microseconds
            );
        }
    }

    // Fallback: use relative time from epoch
    format_relative_timestamp(total_seconds, microseconds)
}

/// Format a relative timestamp (when no absolute datetime is available)
fn format_relative_timestamp(total_seconds: u64, microseconds: u64) -> String {
    // Use 1970-01-01 as base, add the relative seconds
    let secs_per_minute = 60u64;
    let secs_per_hour = 3600u64;
    let secs_per_day = 86400u64;

    let days = total_seconds / secs_per_day;
    let time_of_day = total_seconds % secs_per_day;
    let hours = time_of_day / secs_per_hour;
    let minutes = (time_of_day % secs_per_hour) / secs_per_minute;
    let seconds = time_of_day % secs_per_minute;

    let (year, month, day) = days_to_ymd(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
        year, month, day, hours, minutes, seconds, microseconds
    )
}

/// Parse ISO 8601 datetime string to seconds since Unix epoch (1970-01-01T00:00:00Z)
/// Handles timezone offsets like "+02:00" or "-05:00" by adjusting the result to UTC.
fn parse_datetime_to_epoch(datetime_str: &str) -> Option<u64> {
    // Format: "2024-10-10T18:37:25.559+02:00" or "2024-10-10T18:37:25.559Z"
    // Parse timezone offset if present, then convert local time to UTC

    // Extract timezone offset in seconds (positive = ahead of UTC, negative = behind)
    let tz_offset_secs: i64 = if datetime_str.contains('Z') {
        0 // UTC, no offset
    } else if let Some(plus_pos) = datetime_str.rfind('+') {
        // Positive offset like "+02:00" means local time is ahead of UTC
        parse_tz_offset(&datetime_str[plus_pos + 1..]).unwrap_or(0)
    } else if let Some(minus_pos) = datetime_str.rfind('-') {
        // Check if this is a date separator or timezone offset
        // Timezone offset format: "-HH:MM" at end of string
        let potential_tz = &datetime_str[minus_pos + 1..];
        if potential_tz.contains(':') && potential_tz.len() <= 6 {
            // Negative offset like "-05:00" means local time is behind UTC
            -parse_tz_offset(potential_tz).unwrap_or(0)
        } else {
            0 // Date separator, assume UTC
        }
    } else {
        0 // No timezone info, assume UTC
    };

    // Strip timezone suffix to get clean datetime for parsing
    let datetime_clean = if datetime_str.contains('Z') {
        datetime_str.split('Z').next()?
    } else if datetime_str.contains('+') {
        datetime_str.split('+').next()?
    } else {
        // Handle negative offset: find last '-' that's part of timezone
        let parts: Vec<&str> = datetime_str.rsplitn(2, '-').collect();
        if parts.len() == 2 && parts[0].contains(':') && parts[0].len() <= 5 {
            parts[1]
        } else {
            datetime_str
        }
    };

    let parts: Vec<&str> = datetime_clean.split('T').collect();
    if parts.len() != 2 {
        return None;
    }

    let date_parts: Vec<u32> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    if date_parts.len() != 3 {
        return None;
    }

    let time_part = parts[1].split('.').next()?; // Ignore fractional seconds
    let time_parts: Vec<u32> = time_part
        .split(':')
        .filter_map(|s| s.parse().ok())
        .collect();
    if time_parts.len() != 3 {
        return None;
    }

    let year = date_parts[0];
    let month = date_parts[1];
    let day = date_parts[2];
    let hour = time_parts[0];
    let minute = time_parts[1];
    let second = time_parts[2];

    // Convert to days since epoch (simplified, doesn't handle all edge cases)
    let days = ymd_to_days(year, month, day)?;
    let local_secs =
        (days as u64) * 86400 + (hour as u64) * 3600 + (minute as u64) * 60 + (second as u64);

    // Convert local time to UTC by subtracting the offset
    // If offset is +02:00, local time is 2 hours ahead of UTC, so subtract 2 hours
    let utc_secs = if tz_offset_secs >= 0 {
        local_secs.saturating_sub(tz_offset_secs as u64)
    } else {
        local_secs.saturating_add((-tz_offset_secs) as u64)
    };

    Some(utc_secs)
}

/// Parse timezone offset string like "02:00" or "05:30" to seconds
fn parse_tz_offset(tz_str: &str) -> Option<i64> {
    let parts: Vec<&str> = tz_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hours: i64 = parts[0].parse().ok()?;
    let minutes: i64 = parts[1].parse().ok()?;
    Some(hours * 3600 + minutes * 60)
}

/// Convert year/month/day to days since Unix epoch (1970-01-01)
fn ymd_to_days(year: u32, month: u32, day: u32) -> Option<u64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    // Days in each month (non-leap year)
    let days_in_month = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    let mut total_days: i64 = 0;

    // Add days for complete years since 1970
    for y in 1970..year {
        total_days += if is_leap_year(y) { 366 } else { 365 };
    }

    // Add days for complete months in current year
    for m in 1..month {
        total_days += days_in_month[m as usize] as i64;
        if m == 2 && is_leap_year(year) {
            total_days += 1;
        }
    }

    // Add days in current month
    total_days += (day - 1) as i64;

    if total_days >= 0 {
        Some(total_days as u64)
    } else {
        None
    }
}

/// Convert days since Unix epoch to year/month/day
fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    let mut remaining_days = days as i64;
    let mut year = 1970u32;

    // Find the year
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    // Days in each month (non-leap year)
    let mut days_in_month = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if is_leap_year(year) {
        days_in_month[2] = 29;
    }

    // Find the month
    let mut month = 1u32;
    for (m, &days) in days_in_month.iter().enumerate().skip(1) {
        if remaining_days < days as i64 {
            month = m as u32;
            break;
        }
        remaining_days -= days as i64;
    }

    let day = (remaining_days + 1) as u32;

    (year, month, day)
}

/// Check if a year is a leap year
fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
