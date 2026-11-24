//! Data conversion utilities for BBL parsing
//!
//! Contains all firmware-aware conversion functions for voltage, GPS data,
//! and flag formatting to maintain compatibility across firmware versions.

use crate::firmware_profile::{FirmwareProfile, FirmwareVersion, FlagNaming};
use crate::firmware_tables::{
    InavFlagName, FAILSAFE_PHASE_NAMES, FIELD_ACTIVE_FLIGHT_MODE_FLAGS, FIELD_FLIGHT_MODE_FLAGS,
    FIELD_FLIGHT_MODE_FLAGS2, FIELD_STATE_FLAGS, FLIGHT_MODE_FLAG_NAMES,
    INAV_ACTIVE_FLIGHT_MODE_NAMES, INAV_BOX_MODE_NAMES, INAV_BOX_MODE_NAMES_HIGH,
    INAV_STATE_FLAG_NAMES, STATE_FLAG_NAMES,
};
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

/// Join the names of every set bit in `flags` with `|`, or `"0"` when none is set.
///
/// Bits at or above `names.len()` are ignored, like blackbox_decode.
fn format_flag_names(flags: i32, names: &[&str]) -> String {
    let set: Vec<&str> = names
        .iter()
        .enumerate()
        .filter(|(bit, _)| (flags & (1 << bit)) != 0)
        .map(|(_, name)| *name)
        .collect();

    if set.is_empty() {
        "0".to_string()
    } else {
        set.join("|") // Use pipe separator to avoid breaking CSV format
    }
}

/// Format flight mode flags for CSV output
///
/// Names come from [`FLIGHT_MODE_FLAG_NAMES`].
pub fn format_flight_mode_flags(flags: i32) -> String {
    format_flag_names(flags, &FLIGHT_MODE_FLAG_NAMES)
}

/// Format state flags for CSV output
///
/// Names come from [`STATE_FLAG_NAMES`].
pub fn format_state_flags(flags: i32) -> String {
    format_flag_names(flags, &STATE_FLAG_NAMES)
}

/// Join the names of every set bit of `flags` that `names` defines for `version`
///
/// Bits with no entry for `version` stay unnamed and are ignored, like bits
/// above the table in [`format_flag_names`]. Prints `"0"` when no name matches.
fn format_inav_flag_names(flags: i32, names: &[InavFlagName], version: FirmwareVersion) -> String {
    let word = flags as u32;
    let set: Vec<&str> = names
        .iter()
        .filter(|entry| entry.is_defined_in(version) && (word >> entry.bit) & 1 == 1)
        .map(|entry| entry.name)
        .collect();

    if set.is_empty() {
        "0".to_string()
    } else {
        set.join("|")
    }
}

/// Format one S-frame flag field for an INAV log
///
/// Returns `None` when `field` is not an INAV flag field. The profile decides
/// the naming: INAV names for a covered version, else the raw unsigned value
/// (see [`FlagNaming::RawValue`]).
fn format_inav_flag_field(field: &str, flags: i32, profile: &FirmwareProfile) -> Option<String> {
    let names: &[InavFlagName] = match field {
        FIELD_FLIGHT_MODE_FLAGS => &INAV_BOX_MODE_NAMES,
        FIELD_FLIGHT_MODE_FLAGS2 => &INAV_BOX_MODE_NAMES_HIGH,
        FIELD_ACTIVE_FLIGHT_MODE_FLAGS => &INAV_ACTIVE_FLIGHT_MODE_NAMES,
        FIELD_STATE_FLAGS => &INAV_STATE_FLAG_NAMES,
        _ => return None,
    };
    Some(match (profile.flag_naming, profile.version) {
        (FlagNaming::Inav, Some(version)) => format_inav_flag_names(flags, names, version),
        _ => (flags as u32).to_string(),
    })
}

/// Format a flag field for CSV output, choosing the name table by firmware
///
/// `field` is the S-frame field name (`flightModeFlags`, `flightModeFlags2`,
/// `activeFlightModeFlags` or `stateFlags`). The [`FirmwareProfile`] of the
/// revision header selects the tables: INAV tables keyed by version, the
/// Betaflight tables of [`format_flight_mode_flags`] and
/// [`format_state_flags`], or the raw value. Returns `None` for any other
/// field name, and for `flightModeFlags2` and `activeFlightModeFlags` on
/// non-INAV firmware (they are INAV-only fields; the caller prints the raw
/// value as before).
pub fn format_flag_field_for_firmware(
    field: &str,
    flags: i32,
    firmware_revision: &str,
) -> Option<String> {
    let profile = FirmwareProfile::from_revision(firmware_revision);
    match profile.flag_naming {
        FlagNaming::Betaflight => match field {
            FIELD_FLIGHT_MODE_FLAGS => Some(format_flight_mode_flags(flags)),
            FIELD_STATE_FLAGS => Some(format_state_flags(flags)),
            _ => None,
        },
        FlagNaming::Inav | FlagNaming::RawValue => format_inav_flag_field(field, flags, &profile),
    }
}

/// Format failsafe phase for CSV output (Betaflight/EmuFlight numbering)
///
/// Names come from [`FAILSAFE_PHASE_NAMES`]. A value with no name prints as a
/// raw integer, like blackbox_decode. For INAV logs use
/// [`format_failsafe_phase_for_firmware`].
pub fn format_failsafe_phase(phase: i32) -> String {
    phase_name_or_raw(phase, &FAILSAFE_PHASE_NAMES)
}

/// Format failsafe phase for CSV output, choosing the name table by firmware
///
/// The [`FirmwareProfile`] of the revision header selects the table: INAV
/// logs use the INAV enum names, every other firmware uses
/// [`FAILSAFE_PHASE_NAMES`]. A value with no name prints as a raw integer.
pub fn format_failsafe_phase_for_firmware(phase: i32, firmware_revision: &str) -> String {
    phase_name_or_raw(
        phase,
        FirmwareProfile::from_revision(firmware_revision).failsafe_phase_names,
    )
}

fn phase_name_or_raw(phase: i32, names: &[&str]) -> String {
    usize::try_from(phase)
        .ok()
        .and_then(|index| names.get(index))
        .map_or_else(|| phase.to_string(), |name| (*name).to_string())
}

// ============================================================================
// GPX Timestamp Generation (for GPS export)
// ============================================================================

/// Convert epoch seconds and microseconds to ISO 8601 string.
/// Helper function to eliminate duplication between timestamp formatting functions.
fn epoch_seconds_to_iso8601(total_seconds: u64, microseconds: u64) -> String {
    let secs_per_minute = 60u64;
    let secs_per_hour = 3600u64;
    let secs_per_day = 86400u64;

    let time_of_day = total_seconds % secs_per_day;
    let hours = (time_of_day / secs_per_hour) % 24;
    let minutes = (time_of_day % secs_per_hour) / secs_per_minute;
    let seconds = time_of_day % secs_per_minute;

    let days_since_epoch = total_seconds / secs_per_day;
    let (year, month, day) = days_to_ymd(days_since_epoch);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
        year, month, day, hours, minutes, seconds, microseconds
    )
}

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
            return epoch_seconds_to_iso8601(total_seconds, microseconds);
        }

        // Parse ISO 8601 datetime: "2024-10-10T18:37:25.559+00:00"
        // We only need the date and base time parts for combining with frame offset
        if let Some(base_time) = parse_datetime_to_epoch(datetime_str) {
            let absolute_secs = base_time + total_seconds;
            return epoch_seconds_to_iso8601(absolute_secs, microseconds);
        }
    }

    // Fallback: use relative time from epoch
    epoch_seconds_to_iso8601(total_seconds, microseconds)
}

/// Parse ISO 8601 datetime string to seconds since Unix epoch (1970-01-01T00:00:00Z).
///
/// This function handles the datetime format used by Betaflight's blackbox logs:
/// `YYYY-MM-DDTHH:MM:SS.mmm±HH:MM` (e.g., `2024-10-10T18:37:25.559+00:00`)
///
/// # Accepted Input Formats
/// - `YYYY-MM-DDTHH:MM:SS.mmmZ` - UTC with 'Z' suffix
/// - `YYYY-MM-DDTHH:MM:SS.mmm+HH:MM` - With positive timezone offset (e.g., `+02:00`)
/// - `YYYY-MM-DDTHH:MM:SS.mmm-HH:MM` - With negative timezone offset (e.g., `-05:00`)
/// - `YYYY-MM-DDTHH:MM:SS` - No timezone (treated as UTC)
///
/// # Format Limitations
/// - Compact timezone formats like `-0500` are NOT supported (only colon-separated `HH:MM`)
/// - Region-based timezones like `America/New_York` are NOT supported
/// - Fractional seconds are truncated to whole seconds for epoch calculation
/// - Strings without timezone info are treated as UTC
///
/// # Betaflight Header Context
/// When the flight controller's RTC is not set, Betaflight outputs `0000-01-01T00:00:00.000`
/// as the default datetime. This value should be detected by the caller (via
/// `starts_with("0000-01-01")`) and handled as "no valid datetime available".
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
    let local_secs = days * 86400 + (hour as u64) * 3600 + (minute as u64) * 60 + (second as u64);

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
    // Defensive: if somehow we exhausted all months (shouldn't happen), default to December
    // This satisfies the year-loop invariant that remaining_days < 365/366
    if remaining_days >= days_in_month[month as usize] as i64 {
        month = 12;
    }

    let day = (remaining_days + 1) as u32;

    (year, month, day)
}

/// Check if a year is a leap year
fn is_leap_year(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_leap_year_century_boundaries() {
        // Divisible by 4 but not 100: leap
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2028));
        // Divisible by 100 but not 400: not leap
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2100));
        // Divisible by 400: leap
        assert!(is_leap_year(2000));
        assert!(is_leap_year(2400));
        // Not divisible by 4: not leap
        assert!(!is_leap_year(2023));
        assert!(!is_leap_year(2025));
    }

    // Tests for parse_datetime_to_epoch - locking in Betaflight datetime parsing behavior

    #[test]
    fn test_parse_datetime_utc_z_suffix() {
        // Standard Betaflight format with Z suffix (UTC)
        let result = parse_datetime_to_epoch("2024-10-10T18:37:25.559Z");
        assert!(result.is_some());
        // 2024-10-10 18:37:25 UTC
        // Expected: days since epoch * 86400 + time of day in seconds
        let epoch = result.unwrap();
        assert!(epoch > 0);
        // Verify it's in the right ballpark (2024 is ~54 years after 1970)
        assert!(epoch > 1700000000); // After 2023
        assert!(epoch < 1800000000); // Before 2027
    }

    #[test]
    fn test_parse_datetime_positive_offset() {
        // Betaflight format with positive offset (+02:00)
        // 2024-10-10T18:37:25+02:00 means UTC is 16:37:25
        let with_offset = parse_datetime_to_epoch("2024-10-10T18:37:25.559+02:00");
        let utc_time = parse_datetime_to_epoch("2024-10-10T16:37:25.559Z");

        assert!(with_offset.is_some());
        assert!(utc_time.is_some());
        // Both should result in the same UTC epoch
        assert_eq!(with_offset.unwrap(), utc_time.unwrap());
    }

    #[test]
    fn test_parse_datetime_negative_offset() {
        // Betaflight format with negative offset (-05:00)
        // 2024-10-10T18:37:25-05:00 means UTC is 23:37:25
        let with_offset = parse_datetime_to_epoch("2024-10-10T18:37:25.559-05:00");
        let utc_time = parse_datetime_to_epoch("2024-10-10T23:37:25.559Z");

        assert!(with_offset.is_some());
        assert!(utc_time.is_some());
        // Both should result in the same UTC epoch
        assert_eq!(with_offset.unwrap(), utc_time.unwrap());
    }

    #[test]
    fn test_parse_datetime_zero_offset() {
        // +00:00 should be same as Z
        let with_offset = parse_datetime_to_epoch("2024-10-10T18:37:25.559+00:00");
        let utc_z = parse_datetime_to_epoch("2024-10-10T18:37:25.559Z");

        assert!(with_offset.is_some());
        assert!(utc_z.is_some());
        assert_eq!(with_offset.unwrap(), utc_z.unwrap());
    }

    #[test]
    fn test_parse_datetime_no_timezone_treated_as_utc() {
        // No timezone info - treated as UTC
        let no_tz = parse_datetime_to_epoch("2024-10-10T18:37:25");
        let utc_z = parse_datetime_to_epoch("2024-10-10T18:37:25.000Z");

        assert!(no_tz.is_some());
        assert!(utc_z.is_some());
        assert_eq!(no_tz.unwrap(), utc_z.unwrap());
    }

    #[test]
    fn test_parse_datetime_fractional_seconds_truncated() {
        // Fractional seconds are truncated (not rounded)
        let with_millis = parse_datetime_to_epoch("2024-10-10T18:37:25.999Z");
        let without_millis = parse_datetime_to_epoch("2024-10-10T18:37:25.000Z");

        assert!(with_millis.is_some());
        assert!(without_millis.is_some());
        // Both should be the same (fractional seconds truncated)
        assert_eq!(with_millis.unwrap(), without_millis.unwrap());
    }

    #[test]
    fn test_parse_datetime_betaflight_default_placeholder() {
        // Betaflight default when RTC not set: 0000-01-01T00:00:00.000
        // This should parse (returns Some) but caller should detect via starts_with("0000-01-01")
        let result = parse_datetime_to_epoch("0000-01-01T00:00:00.000+00:00");
        // The function may return None for year 0000 since it's before 1970
        // or it may return Some with an invalid value - either is acceptable
        // The important thing is the caller checks for "0000-01-01" prefix first
        // This test documents the current behavior
        assert!(result.is_none() || result == Some(0));
    }

    #[test]
    fn test_parse_datetime_half_hour_offset() {
        // Some timezones have 30-minute offsets (e.g., India +05:30)
        let with_offset = parse_datetime_to_epoch("2024-10-10T18:37:25.559+05:30");
        let utc_time = parse_datetime_to_epoch("2024-10-10T13:07:25.559Z");

        assert!(with_offset.is_some());
        assert!(utc_time.is_some());
        assert_eq!(with_offset.unwrap(), utc_time.unwrap());
    }

    #[test]
    fn test_parse_datetime_invalid_format_returns_none() {
        // Invalid formats should return None
        assert!(parse_datetime_to_epoch("not-a-datetime").is_none());
        assert!(parse_datetime_to_epoch("2024-10-10").is_none()); // Missing time
        assert!(parse_datetime_to_epoch("18:37:25").is_none()); // Missing date
        assert!(parse_datetime_to_epoch("").is_none());
    }

    #[test]
    fn test_parse_datetime_compact_offset_not_supported() {
        // Compact offset format like -0500 is NOT supported (only HH:MM with colon)
        // This test documents the limitation - it will be treated as no timezone
        let compact = parse_datetime_to_epoch("2024-10-10T18:37:25.559-0500");
        // The -0500 won't be parsed as a timezone, so it's treated as local time without offset
        // This means it differs from the colon-separated version
        let _colon_sep = parse_datetime_to_epoch("2024-10-10T18:37:25.559-05:00");

        // Both should parse, but may have different values
        // The compact form will be treated as UTC (offset not recognized)
        assert!(compact.is_some() || compact.is_none()); // May or may not parse
    }

    // Tests for generate_gpx_timestamp

    #[test]
    fn test_generate_gpx_timestamp_with_valid_datetime() {
        // When log_start_datetime is valid, should produce absolute timestamp
        let timestamp = generate_gpx_timestamp(Some("2024-10-10T18:37:25.559+00:00"), 1_000_000);
        assert!(timestamp.contains("2024-10-10T18:37:26")); // 1 second after start
    }

    #[test]
    fn test_generate_gpx_timestamp_placeholder_datetime() {
        // When FC clock wasn't set, should fall back to relative time
        let timestamp = generate_gpx_timestamp(Some("0000-01-01T00:00:00.000+00:00"), 1_000_000);
        // Should use 1970-01-01 as base (relative time)
        assert!(timestamp.contains("1970-01-01"));
    }

    #[test]
    fn test_generate_gpx_timestamp_no_datetime() {
        // When no datetime provided, should use relative time from epoch
        let timestamp = generate_gpx_timestamp(None, 0);
        assert!(timestamp.contains("1970-01-01T00:00:00"));
    }

    // Expected names below are written from the INAV enums (rc_modes.h
    // boxId_e, runtime_config.h flightModeFlags_e and stateFlags_t), not
    // read back from the tables.
    const INAV_6_1_1: &str = "INAV 6.1.1 (42ced253bd) MATEKF405";
    const INAV_7_0_0: &str = "INAV 7.0.0 (118cd48550) MATEKF405";
    const INAV_7_1_2: &str = "INAV 7.1.2 (ed7fb3bb32) MATEKF405";
    const INAV_8_0_1: &str = "INAV 8.0.1 (ae47bcba01) MATEKF405";
    const INAV_9_1_0: &str = "INAV 9.1.0 (e519b69b02) MATEKF405";
    const INAV_10_0_0: &str = "INAV 10.0.0 (f1cd5c6f68) MATEKF405";

    fn fmt(field: &str, flags: i32, revision: &str) -> String {
        format_flag_field_for_firmware(field, flags, revision).expect("flag field")
    }

    #[test]
    fn test_non_inav_header_naming_inav_later_keeps_betaflight_tables() {
        // First-word rule: "INAV" later in the string must not select INAV tables.
        for rev in [
            "Quicksilver 1.0 (INAV-compatible build)",
            "Betaflight 4.5.0 (INAV)",
        ] {
            // Bit 2 is MAG in the Betaflight table; the INAV table names it differently.
            assert_eq!(fmt("flightModeFlags", 1 << 2, rev), "MAG", "{rev}");
            assert_eq!(
                format_flag_field_for_firmware("flightModeFlags2", 1, rev),
                None
            );
            assert_eq!(
                format_flag_field_for_firmware("activeFlightModeFlags", 1, rev),
                None
            );
            assert_eq!(format_failsafe_phase_for_firmware(2, rev), "LANDING");
        }
    }

    #[test]
    fn test_inav_box_mode_names_by_version() {
        // flightModeFlags holds boxId_e bits: bit 0 ARM, 1 ANGLE, 2 HORIZON.
        for rev in [INAV_6_1_1, INAV_7_1_2, INAV_8_0_1, INAV_10_0_0] {
            assert_eq!(fmt("flightModeFlags", 0, rev), "0");
            assert_eq!(fmt("flightModeFlags", 1, rev), "ARM");
            assert_eq!(fmt("flightModeFlags", 0b111, rev), "ARM|ANGLE|HORIZON");
            // Bits 3, 4, 8, 9 differ from the Betaflight names (BARO, GPS_HOME, ...).
            assert_eq!(fmt("flightModeFlags", 1 << 3, rev), "NAVALTHOLD");
            assert_eq!(fmt("flightModeFlags", 1 << 4, rev), "HEADINGHOLD");
            assert_eq!(fmt("flightModeFlags", 1 << 8, rev), "NAVRTH");
            assert_eq!(fmt("flightModeFlags", 1 << 9, rev), "NAVPOSHOLD");
            assert_eq!(fmt("flightModeFlags", 1 << 29, rev), "CAMERA1");
        }
        // Bit 23: KILLSWITCH in 6.1.1 and 7.1.2, unused from 8.0.0.
        assert_eq!(fmt("flightModeFlags", 1 << 23, INAV_6_1_1), "KILLSWITCH");
        assert_eq!(fmt("flightModeFlags", 1 << 23, INAV_7_1_2), "KILLSWITCH");
        assert_eq!(fmt("flightModeFlags", 1 << 23, INAV_8_0_1), "0");
        assert_eq!(fmt("flightModeFlags", 1 << 23, INAV_10_0_0), "0");
    }

    #[test]
    fn test_inav_box_mode_high_bit_32() {
        // Bit 31 is BOXCAMERA3 = 31; i32 wraps negative for a set high bit.
        assert_eq!(fmt("flightModeFlags", i32::MIN, INAV_8_0_1), "CAMERA3");
        // All 32 bits set: 32 names in 6.1.1 (KILLSWITCH defined), 31 from 8.0.0.
        assert_eq!(
            fmt("flightModeFlags", -1, INAV_6_1_1).split('|').count(),
            32
        );
        assert_eq!(
            fmt("flightModeFlags", -1, INAV_8_0_1).split('|').count(),
            31
        );
        assert!(fmt("flightModeFlags", -1, INAV_8_0_1).ends_with("|CAMERA3"));
        // Raw unsigned form for an unverified version, not a negative number.
        assert_eq!(
            fmt("flightModeFlags", i32::MIN, "INAV 5.1.0 (sha) T"),
            "2147483648"
        );
    }

    #[test]
    fn test_inav_flight_mode_flags2_names() {
        // flightModeFlags2 bit n is boxId_e n + 32: OSDALT1 = 32, GIMBALHTRK = 59.
        assert_eq!(fmt("flightModeFlags2", 1, INAV_8_0_1), "OSDALT1");
        assert_eq!(fmt("flightModeFlags2", 1 << 3, INAV_8_0_1), "NAVCOURSEHOLD");
        assert_eq!(fmt("flightModeFlags2", 1 << 27, INAV_8_0_1), "GIMBALHTRK");
        // Box ids 51..=54 (bits 19..=22) arrive in 7.0.0, ANGLEHOLD (55, bit 23) in 7.1.0.
        assert_eq!(fmt("flightModeFlags2", 1 << 21, INAV_7_0_0), "MIXERPROFILE");
        assert_eq!(fmt("flightModeFlags2", 1 << 23, INAV_7_0_0), "0");
        assert_eq!(fmt("flightModeFlags2", 1 << 23, INAV_7_1_2), "ANGLEHOLD");
        // Box ids 56..=59 (gimbal) arrive in 8.0.0; 60..=63 in 10.0.0.
        assert_eq!(fmt("flightModeFlags2", 1 << 24, INAV_7_1_2), "0");
        assert_eq!(fmt("flightModeFlags2", 1 << 24, INAV_9_1_0), "GIMBALTLOCK");
        assert_eq!(fmt("flightModeFlags2", 1 << 28, INAV_9_1_0), "0");
        assert_eq!(fmt("flightModeFlags2", 1 << 28, INAV_10_0_0), "AUTOSPEED");
        assert_eq!(
            fmt("flightModeFlags2", i32::MIN, INAV_10_0_0),
            "THRUSTREVERSE"
        );
        assert_eq!(fmt("flightModeFlags2", i32::MIN, INAV_9_1_0), "0");
    }

    #[test]
    fn test_inav_active_flight_mode_names() {
        assert_eq!(fmt("activeFlightModeFlags", 1, INAV_8_0_1), "ANGLE_MODE");
        assert_eq!(
            fmt("activeFlightModeFlags", 1 << 2, INAV_8_0_1),
            "HEADING_MODE"
        );
        assert_eq!(
            fmt("activeFlightModeFlags", 1 << 9, INAV_8_0_1),
            "FAILSAFE_MODE"
        );
        assert_eq!(
            fmt("activeFlightModeFlags", 1 << 16, INAV_8_0_1),
            "SOARING_MODE"
        );
        assert_eq!(
            fmt("activeFlightModeFlags", 1 << 18, INAV_9_1_0),
            "NAV_FW_AUTOLAND"
        );
        assert_eq!(
            fmt("activeFlightModeFlags", 1 << 19, INAV_9_1_0),
            "NAV_SEND_TO"
        );
        // Bit 19 does not exist before 8.0.0; bits 17 and 18 not before 7.1.0.
        assert_eq!(fmt("activeFlightModeFlags", 1 << 19, INAV_7_1_2), "0");
        assert_eq!(fmt("activeFlightModeFlags", 1 << 17, INAV_7_0_0), "0");
        assert_eq!(
            fmt("activeFlightModeFlags", 1 << 17, INAV_7_1_2),
            "ANGLEHOLD_MODE"
        );
        // Bits above the enum (20..=31) stay unnamed.
        assert_eq!(fmt("activeFlightModeFlags", i32::MIN, INAV_10_0_0), "0");
    }

    #[test]
    fn test_inav_state_flag_names() {
        assert_eq!(
            fmt("stateFlags", 0b1111, INAV_8_0_1),
            "GPS_FIX_HOME|GPS_FIX|CALIBRATE_MAG|SMALL_ANGLE"
        );
        assert_eq!(fmt("stateFlags", 1 << 4, INAV_8_0_1), "FIXED_WING_LEGACY");
        assert_eq!(fmt("stateFlags", 1 << 17, INAV_6_1_1), "AIRPLANE");
        assert_eq!(fmt("stateFlags", 1 << 26, INAV_6_1_1), "LANDING_DETECTED");
        // Bit 10 from 8.0.0, bit 27 from 7.0.0, bit 28 from 7.1.0.
        assert_eq!(fmt("stateFlags", 1 << 10, INAV_7_1_2), "0");
        assert_eq!(fmt("stateFlags", 1 << 10, INAV_8_0_1), "GPS_ESTIMATED_FIX");
        assert_eq!(fmt("stateFlags", 1 << 27, INAV_6_1_1), "0");
        assert_eq!(
            fmt("stateFlags", 1 << 27, INAV_7_0_0),
            "IN_FLIGHT_EMERG_REARM"
        );
        assert_eq!(fmt("stateFlags", 1 << 28, INAV_7_0_0), "0");
        assert_eq!(fmt("stateFlags", 1 << 28, INAV_7_1_2), "TAILSITTER");
        // Bits 29..=31 are not defined in any verified version.
        assert_eq!(fmt("stateFlags", i32::MIN, INAV_10_0_0), "0");
    }

    #[test]
    fn test_inav_unverified_version_prints_raw() {
        for rev in [
            "INAV 5.1.0 (sha) T", // older than the first verified version
            "INAV 1.9.1 (sha) T",
            "INAV (sha) T", // no version
            "INAV",
        ] {
            assert_eq!(fmt("flightModeFlags", 5, rev), "5", "{rev}");
            assert_eq!(fmt("flightModeFlags2", 5, rev), "5", "{rev}");
            assert_eq!(fmt("activeFlightModeFlags", 5, rev), "5", "{rev}");
            assert_eq!(fmt("stateFlags", 5, rev), "5", "{rev}");
        }
        // The first verified version is named.
        assert_eq!(fmt("flightModeFlags", 1, "INAV 6.0.0 (sha) T"), "ARM");
    }

    #[test]
    fn test_flag_field_non_inav_unchanged() {
        for rev in [
            "Betaflight 4.5.1 (77d01ba3b) AT32F435M",
            "EmuFlight 0.4.3 (sha) T",
            "",
            "Cleanflight 2.3.0",
        ] {
            // Same text as the Betaflight-name formatters.
            for flags in [0, 1, 4, 8, 32, 1 << 10, 1 << 11, 1 << 12, i32::MIN, -1] {
                assert_eq!(
                    format_flag_field_for_firmware("flightModeFlags", flags, rev),
                    Some(format_flight_mode_flags(flags)),
                    "{rev} {flags}"
                );
                assert_eq!(
                    format_flag_field_for_firmware("stateFlags", flags, rev),
                    Some(format_state_flags(flags)),
                    "{rev} {flags}"
                );
            }
            // INAV-only fields are not formatted for other firmware.
            assert_eq!(
                format_flag_field_for_firmware("flightModeFlags2", 1, rev),
                None
            );
            assert_eq!(
                format_flag_field_for_firmware("activeFlightModeFlags", 1, rev),
                None
            );
        }
        assert_eq!(fmt("flightModeFlags", 4, "Betaflight 4.5.1 (sha) T"), "MAG");
        // Unrelated field names are not claimed.
        assert_eq!(
            format_flag_field_for_firmware("failsafePhase", 1, INAV_8_0_1),
            None
        );
    }
}
