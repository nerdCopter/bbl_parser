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
