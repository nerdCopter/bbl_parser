//! BBL Parser Library
//!
//! A Rust library for parsing Betaflight/EmuFlight/INAV blackbox log files.
//! This library provides both in-memory data access and export capabilities.
//!
//! # Examples
//!
//! Basic usage:
//! ```rust,no_run
//! use bbl_parser::parse_bbl_file;
//! use std::path::Path;
//!
//! let export_options = bbl_parser::ExportOptions::default();
//! let log = parse_bbl_file(Path::new("flight.BBL"), export_options, false).unwrap();
//! println!("Found {} frames", log.sample_frames.len());
//! ```

mod bbl_format;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Field definition for BBL frame parsing
#[derive(Debug, Clone)]
pub struct FieldDefinition {
    pub name: String,
    pub signed: bool,
    pub predictor: u8,
    pub encoding: u8,
}

/// Frame definition containing field information
#[derive(Debug, Clone)]
pub struct FrameDefinition {
    pub fields: Vec<FieldDefinition>,
    pub field_names: Vec<String>,
    pub count: usize,
}

impl FrameDefinition {
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            field_names: Vec::new(),
            count: 0,
        }
    }

    pub fn from_field_names(names: Vec<String>) -> Self {
        let fields = names
            .iter()
            .map(|name| FieldDefinition {
                name: name.clone(),
                signed: false,
                predictor: 0,
                encoding: 0,
            })
            .collect();
        let count = names.len();
        Self {
            fields,
            field_names: names,
            count,
        }
    }

    pub fn update_predictors(&mut self, predictors: &[i32]) {
        for (i, predictor) in predictors.iter().enumerate() {
            if i < self.fields.len() {
                self.fields[i].predictor = *predictor as u8;
            }
        }
    }

    pub fn update_encoding(&mut self, encodings: &[i32]) {
        for (i, encoding) in encodings.iter().enumerate() {
            if i < self.fields.len() {
                self.fields[i].encoding = *encoding as u8;
            }
        }
    }

    pub fn update_signed(&mut self, signed_values: &[i32]) {
        for (i, signed) in signed_values.iter().enumerate() {
            if i < self.fields.len() {
                self.fields[i].signed = *signed != 0;
            }
        }
    }
}

impl Default for FrameDefinition {
    fn default() -> Self {
        Self::new()
    }
}

/// BBL header information containing all metadata and frame definitions
#[derive(Debug, Clone)]
pub struct BBLHeader {
    pub firmware_revision: String,
    pub board_info: String,
    pub craft_name: String,
    pub data_version: u8,
    pub looptime: u32,
    pub i_frame_def: FrameDefinition,
    pub p_frame_def: FrameDefinition,
    pub s_frame_def: FrameDefinition,
    pub g_frame_def: FrameDefinition,
    pub h_frame_def: FrameDefinition,
    pub sysconfig: HashMap<String, i32>,
    pub all_headers: Vec<String>,
}

impl BBLHeader {
    pub fn new() -> Self {
        Self {
            firmware_revision: String::new(),
            board_info: String::new(),
            craft_name: String::new(),
            data_version: 2,
            looptime: 0,
            i_frame_def: FrameDefinition::new(),
            p_frame_def: FrameDefinition::new(),
            s_frame_def: FrameDefinition::new(),
            g_frame_def: FrameDefinition::new(),
            h_frame_def: FrameDefinition::new(),
            sysconfig: HashMap::new(),
            all_headers: Vec::new(),
        }
    }
}

impl Default for BBLHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// A decoded frame with timestamp and flight data
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    pub frame_type: char,
    pub timestamp_us: u64,
    pub loop_iteration: u32,
    pub data: HashMap<String, i32>,
}

/// GPS coordinate data
#[derive(Debug, Clone)]
pub struct GpsCoordinate {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
    pub timestamp_us: u64,
    pub num_sats: Option<i32>,
    pub speed: Option<f64>,
    pub ground_course: Option<f64>,
}

/// GPS home coordinate data
#[derive(Debug, Clone)]
pub struct GpsHomeCoordinate {
    pub home_latitude: f64,
    pub home_longitude: f64,
    pub timestamp_us: u64,
}

/// Flight event data
#[derive(Debug, Clone)]
pub struct EventFrame {
    pub timestamp_us: u64,
    pub event_type: u8,
    pub event_data: Vec<u8>,
    pub event_description: String,
}

/// Frame parsing statistics
#[derive(Debug, Default)]
pub struct FrameStats {
    pub i_frames: u32,
    pub p_frames: u32,
    pub h_frames: u32,
    pub g_frames: u32,
    pub e_frames: u32,
    pub s_frames: u32,
    pub total_frames: u32,
    pub total_bytes: u64,
    pub start_time_us: u64,
    pub end_time_us: u64,
    pub failed_frames: u32,
    pub missing_iterations: u64,
}

/// Main BBL log data structure containing all parsed information
#[derive(Debug)]
pub struct BBLLog {
    pub log_number: usize,
    pub total_logs: usize,
    pub header: BBLHeader,
    pub stats: FrameStats,
    pub sample_frames: Vec<DecodedFrame>,
    pub debug_frames: Option<HashMap<char, Vec<DecodedFrame>>>,
    pub gps_coordinates: Vec<GpsCoordinate>,
    pub home_coordinates: Vec<GpsHomeCoordinate>,
    pub event_frames: Vec<EventFrame>,
}

impl BBLLog {
    /// Get all frames of a specific type
    pub fn get_frames_by_type(&self, frame_type: char) -> Option<&Vec<DecodedFrame>> {
        self.debug_frames.as_ref()?.get(&frame_type)
    }

    /// Get frames in a specific time range (microseconds)
    pub fn get_frames_in_time_range(&self, start_us: u64, end_us: u64) -> Vec<&DecodedFrame> {
        let mut result = Vec::new();

        if let Some(ref debug_frames) = self.debug_frames {
            for frames in debug_frames.values() {
                for frame in frames {
                    if frame.timestamp_us >= start_us && frame.timestamp_us <= end_us {
                        result.push(frame);
                    }
                }
            }
        }

        // Sort by timestamp
        result.sort_by_key(|f| f.timestamp_us);
        result
    }

    /// Get all I and P frames sorted by timestamp
    pub fn get_main_frames(&self) -> Vec<&DecodedFrame> {
        let mut frames = Vec::new();

        if let Some(ref debug_frames) = self.debug_frames {
            for frame_type in ['I', 'P'] {
                if let Some(type_frames) = debug_frames.get(&frame_type) {
                    frames.extend(type_frames.iter());
                }
            }
        }

        // Sort by timestamp
        frames.sort_by_key(|f| f.timestamp_us);
        frames
    }

    /// Extract gyro data from frames
    pub fn get_gyro_data(&self) -> Vec<[f32; 3]> {
        let frames = self.get_main_frames();
        let mut gyro_data = Vec::new();

        for frame in frames {
            if let (Some(&x), Some(&y), Some(&z)) = (
                frame.data.get("gyroADC[0]"),
                frame.data.get("gyroADC[1]"),
                frame.data.get("gyroADC[2]"),
            ) {
                gyro_data.push([x as f32, y as f32, z as f32]);
            }
        }

        gyro_data
    }

    /// Extract PID data from frames
    pub fn get_pid_data(&self) -> Vec<[f32; 3]> {
        let frames = self.get_main_frames();
        let mut pid_data = Vec::new();

        for frame in frames {
            if let (Some(&x), Some(&y), Some(&z)) = (
                frame.data.get("axisP[0]"),
                frame.data.get("axisP[1]"),
                frame.data.get("axisP[2]"),
            ) {
                pid_data.push([x as f32, y as f32, z as f32]);
            }
        }

        pid_data
    }
}

/// Export options for different output formats
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    pub csv: bool,
    pub gpx: bool,
    pub event: bool,
    pub output_dir: Option<String>,
}

/// Parse a BBL file from disk
///
/// # Arguments
/// * `file_path` - Path to the BBL file
/// * `export_options` - Export configuration
/// * `debug` - Enable debug output
///
/// # Returns
/// * `Result<BBLLog, anyhow::Error>` - Parsed log data or error
pub fn parse_bbl_file(
    file_path: &Path,
    _export_options: ExportOptions,
    debug: bool,
) -> Result<BBLLog> {
    if debug {
        println!("Reading file: {:?}", file_path);
    }

    let _file_data = std::fs::read(file_path)
        .with_context(|| format!("Failed to read BBL file: {:?}", file_path))?;

    // For now, return a minimal log structure
    // TODO: Implement actual parsing by moving functionality from main.rs
    Ok(BBLLog {
        log_number: 1,
        total_logs: 1,
        header: BBLHeader::default(),
        stats: FrameStats::default(),
        sample_frames: Vec::new(),
        debug_frames: None,
        gps_coordinates: Vec::new(),
        home_coordinates: Vec::new(),
        event_frames: Vec::new(),
    })
}

/// Parse BBL data from memory
///
/// # Arguments
/// * `data` - BBL file data as bytes
/// * `export_options` - Export configuration
/// * `debug` - Enable debug output
///
/// # Returns
/// * `Result<BBLLog, anyhow::Error>` - Parsed log data or error
pub fn parse_bbl_bytes(
    _data: &[u8],
    _export_options: ExportOptions,
    _debug: bool,
) -> Result<BBLLog> {
    // TODO: Implement actual parsing by moving functionality from main.rs
    Ok(BBLLog {
        log_number: 1,
        total_logs: 1,
        header: BBLHeader::default(),
        stats: FrameStats::default(),
        sample_frames: Vec::new(),
        debug_frames: None,
        gps_coordinates: Vec::new(),
        home_coordinates: Vec::new(),
        event_frames: Vec::new(),
    })
}

/// Export log data to CSV format
#[cfg(feature = "csv")]
pub fn export_to_csv(_log: &BBLLog, _output_path: &Path) -> Result<()> {
    // TODO: Implement CSV export by moving functionality from main.rs
    Ok(())
}

// Re-export commonly used types
pub use bbl_format::*;

// Helper functions that need to be moved from main.rs

/// Convert voltage readings to volts with firmware-specific scaling
pub fn convert_vbat_to_volts(raw_value: i32, firmware_info: &str) -> f64 {
    // Temporary implementation - real logic needs to be moved from main.rs
    if firmware_info.contains("Betaflight") && firmware_info.contains("4.") {
        // Check if version >= 4.3.0 for hundredths vs tenths
        if let Some(version_start) = firmware_info.find("4.") {
            if let Some(version_part) = firmware_info.get(version_start..version_start + 5) {
                if version_part >= "4.3.0" {
                    return raw_value as f64 / 100.0; // hundredths
                }
            }
        }
        raw_value as f64 / 10.0 // tenths for older versions
    } else if firmware_info.contains("EmuFlight") {
        raw_value as f64 / 10.0 // always tenths
    } else if firmware_info.contains("iNav") {
        raw_value as f64 / 100.0 // always hundredths
    } else {
        raw_value as f64 / 10.0 // default to tenths
    }
}

/// Convert amperage readings to amps (0.01A units)
pub fn convert_amperage_to_amps(raw_value: i32) -> f64 {
    raw_value as f64 * 0.01
}

/// Format flight mode flags according to Betaflight enum
pub fn format_flight_mode_flags(flags: i32) -> String {
    if flags == 0 {
        return "0".to_string();
    }

    let mut mode_names = Vec::new();

    if flags & (1 << 0) != 0 {
        mode_names.push("ANGLE_MODE");
    }
    if flags & (1 << 1) != 0 {
        mode_names.push("HORIZON_MODE");
    }
    if flags & (1 << 2) != 0 {
        mode_names.push("MAG");
    }
    if flags & (1 << 3) != 0 {
        mode_names.push("BARO");
    }
    if flags & (1 << 5) != 0 {
        mode_names.push("GPS_HOLD");
    }
    if flags & (1 << 6) != 0 {
        mode_names.push("HEADFREE");
    }
    if flags & (1 << 8) != 0 {
        mode_names.push("PASSTHRU");
    }
    if flags & (1 << 10) != 0 {
        mode_names.push("FAILSAFE_MODE");
    }
    if flags & (1 << 11) != 0 {
        mode_names.push("GPS_RESCUE_MODE");
    }

    if mode_names.is_empty() {
        flags.to_string()
    } else {
        mode_names.join("|")
    }
}

/// Format state flags according to Betaflight enum
pub fn format_state_flags(flags: i32) -> String {
    if flags == 0 {
        return "0".to_string();
    }

    let mut state_names = Vec::new();

    if flags & (1 << 0) != 0 {
        state_names.push("GPS_FIX_HOME");
    }
    if flags & (1 << 1) != 0 {
        state_names.push("GPS_FIX");
    }
    if flags & (1 << 2) != 0 {
        state_names.push("CALIBRATE_MAG");
    }
    if flags & (1 << 3) != 0 {
        state_names.push("SMALL_ANGLE");
    }
    if flags & (1 << 4) != 0 {
        state_names.push("FIXED_WING");
    }

    if state_names.is_empty() {
        flags.to_string()
    } else {
        state_names.join("|")
    }
}

/// Format failsafe phase according to Betaflight enum
pub fn format_failsafe_phase(phase: i32) -> String {
    match phase {
        0 => "IDLE".to_string(),
        1 => "RX_LOSS_DETECTED".to_string(),
        2 => "LANDING".to_string(),
        3 => "LANDED".to_string(),
        4 => "RX_LOSS_MONITORING".to_string(),
        5 => "RX_LOSS_RECOVERED".to_string(),
        6 => "GPS_RESCUE".to_string(),
        _ => phase.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_frame_definition_creation() {
        let mut frame_def = FrameDefinition::new();
        assert_eq!(frame_def.count, 0);
        assert!(frame_def.field_names.is_empty());

        let field_names = vec!["time".to_string(), "loopIteration".to_string()];
        frame_def = FrameDefinition::from_field_names(field_names.clone());
        assert_eq!(frame_def.count, 2);
        assert_eq!(frame_def.field_names, field_names);
    }

    #[test]
    fn test_frame_definition_predictor_update() {
        let mut frame_def =
            FrameDefinition::from_field_names(vec!["field1".to_string(), "field2".to_string()]);
        let predictors = vec![1, 2];
        frame_def.update_predictors(&predictors);

        assert_eq!(frame_def.fields[0].predictor, 1);
        assert_eq!(frame_def.fields[1].predictor, 2);
    }

    #[test]
    fn test_unit_conversions() {
        // Test voltage conversion with firmware-aware scaling

        // Test Betaflight >= 4.3.0 (hundredths)
        let volts_bf_new = convert_vbat_to_volts(1365, "Betaflight 4.5.1 (77d01ba3b) AT32F435M");
        assert!((volts_bf_new - 13.65).abs() < 0.01); // Should be 13.65V (hundredths)

        // Test Betaflight < 4.3.0 (tenths)
        let volts_bf_old = convert_vbat_to_volts(136, "Betaflight 4.2.0 (abc123) STM32F7X2");
        assert!((volts_bf_old - 13.6).abs() < 0.01); // Should be 13.6V (tenths)

        // Test EmuFlight (always tenths)
        let volts_emuf = convert_vbat_to_volts(136, "EmuFlight 0.3.5 (abc123) STM32F7X2");
        assert!((volts_emuf - 13.6).abs() < 0.01); // Should be 13.6V (tenths)

        // Test iNav (always hundredths)
        let volts_inav = convert_vbat_to_volts(1365, "iNav 7.1.0 (abc123) STM32F7X2");
        assert!((volts_inav - 13.65).abs() < 0.01); // Should be 13.65V (hundredths)

        // Test amperage conversion (0.01A units)
        let amps = convert_amperage_to_amps(100); // 100 * 0.01 = 1.0A
        assert!((amps - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_frame_stats_default() {
        let stats = FrameStats::default();
        assert_eq!(stats.total_frames, 0);
        assert_eq!(stats.i_frames, 0);
        assert_eq!(stats.p_frames, 0);
        assert_eq!(stats.failed_frames, 0);
    }

    #[test]
    fn test_csv_export_options() {
        let options = ExportOptions {
            csv: true,
            gpx: false,
            event: false,
            output_dir: Some("/tmp".to_string()),
        };
        assert_eq!(options.output_dir.as_ref().unwrap(), "/tmp");

        let options = ExportOptions {
            csv: false,
            gpx: false,
            event: false,
            output_dir: None,
        };
        assert!(options.output_dir.is_none());
    }

    #[test]
    fn test_file_extension_validation() {
        let valid_extensions = ["bbl", "bfl", "txt"];
        let invalid_extensions = ["csv", "json", "xml"];

        for ext in valid_extensions {
            let path = PathBuf::from(format!("test.{ext}"));
            let is_valid = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| {
                    let ext_lower = e.to_ascii_lowercase();
                    ext_lower == "bbl" || ext_lower == "bfl" || ext_lower == "txt"
                })
                .unwrap_or(false);
            assert!(is_valid, "Extension {ext} should be valid");
        }

        for ext in invalid_extensions {
            let path = PathBuf::from(format!("test.{ext}"));
            let is_valid = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| {
                    let ext_lower = e.to_ascii_lowercase();
                    ext_lower == "bbl" || ext_lower == "bfl" || ext_lower == "txt"
                })
                .unwrap_or(false);
            assert!(!is_valid, "Extension {ext} should be invalid");
        }
    }

    #[test]
    fn test_bbl_header_creation() {
        let header = BBLHeader {
            firmware_revision: "4.5.0".to_string(),
            board_info: "MAMBAF722".to_string(),
            craft_name: "TestCraft".to_string(),
            data_version: 2,
            looptime: 500,
            i_frame_def: FrameDefinition::new(),
            p_frame_def: FrameDefinition::new(),
            s_frame_def: FrameDefinition::new(),
            g_frame_def: FrameDefinition::new(),
            h_frame_def: FrameDefinition::new(),
            sysconfig: HashMap::new(),
            all_headers: Vec::new(),
        };

        assert_eq!(header.firmware_revision, "4.5.0");
        assert_eq!(header.board_info, "MAMBAF722");
        assert_eq!(header.craft_name, "TestCraft");
        assert_eq!(header.data_version, 2);
        assert_eq!(header.looptime, 500);
    }

    #[test]
    fn test_decoded_frame_creation() {
        let mut data = HashMap::new();
        data.insert("time".to_string(), 1000);
        data.insert("loopIteration".to_string(), 1);

        let frame = DecodedFrame {
            frame_type: 'I',
            timestamp_us: 1000,
            loop_iteration: 1,
            data,
        };

        assert_eq!(frame.frame_type, 'I');
        assert_eq!(frame.timestamp_us, 1000);
        assert_eq!(frame.loop_iteration, 1);
        assert_eq!(frame.data.get("time"), Some(&1000));
    }

    #[test]
    fn test_format_flight_mode_flags() {
        // Test no flags
        assert_eq!(format_flight_mode_flags(0), "0");

        // Test single flags - matches Betaflight flightModeFlags_e enum
        assert_eq!(format_flight_mode_flags(1), "ANGLE_MODE"); // bit 0 = ANGLE_MODE
        assert_eq!(format_flight_mode_flags(2), "HORIZON_MODE"); // bit 1 = HORIZON_MODE
        assert_eq!(format_flight_mode_flags(4), "MAG"); // bit 2 = MAG_MODE
        assert_eq!(format_flight_mode_flags(8), "BARO"); // bit 3 = ALT_HOLD_MODE (old name BARO)
        assert_eq!(format_flight_mode_flags(32), "GPS_HOLD"); // bit 5 = POS_HOLD_MODE (old name GPS_HOLD)
        assert_eq!(format_flight_mode_flags(64), "HEADFREE"); // bit 6 = HEADFREE_MODE
        assert_eq!(format_flight_mode_flags(256), "PASSTHRU"); // bit 8 = PASSTHRU_MODE
        assert_eq!(format_flight_mode_flags(1024), "FAILSAFE_MODE"); // bit 10 = FAILSAFE_MODE
        assert_eq!(format_flight_mode_flags(2048), "GPS_RESCUE_MODE"); // bit 11 = GPS_RESCUE_MODE

        // Test multiple flags (pipe-separated to avoid breaking CSV format)
        assert_eq!(format_flight_mode_flags(3), "ANGLE_MODE|HORIZON_MODE"); // bits 0+1
        assert_eq!(format_flight_mode_flags(6), "HORIZON_MODE|MAG"); // bits 1+2
        assert_eq!(format_flight_mode_flags(7), "ANGLE_MODE|HORIZON_MODE|MAG"); // bits 0+1+2
    }

    #[test]
    fn test_format_state_flags() {
        // Test no flags
        assert_eq!(format_state_flags(0), "0");

        // Test single flags - matches Betaflight stateFlags_t enum
        assert_eq!(format_state_flags(1), "GPS_FIX_HOME"); // bit 0 = GPS_FIX_HOME
        assert_eq!(format_state_flags(2), "GPS_FIX"); // bit 1 = GPS_FIX
        assert_eq!(format_state_flags(4), "CALIBRATE_MAG"); // bit 2 = GPS_FIX_EVER (old name)
        assert_eq!(format_state_flags(8), "SMALL_ANGLE"); // bit 3 = compatibility
        assert_eq!(format_state_flags(16), "FIXED_WING"); // bit 4 = compatibility

        // Test multiple flags (pipe-separated to avoid breaking CSV format)
        assert_eq!(format_state_flags(3), "GPS_FIX_HOME|GPS_FIX"); // bits 0+1
        assert_eq!(format_state_flags(7), "GPS_FIX_HOME|GPS_FIX|CALIBRATE_MAG");
        // bits 0+1+2
    }

    #[test]
    fn test_format_failsafe_phase() {
        // Test known phases - matches Betaflight failsafePhase_e enum
        assert_eq!(format_failsafe_phase(0), "IDLE"); // FAILSAFE_IDLE
        assert_eq!(format_failsafe_phase(1), "RX_LOSS_DETECTED"); // FAILSAFE_RX_LOSS_DETECTED
        assert_eq!(format_failsafe_phase(2), "LANDING"); // FAILSAFE_LANDING
        assert_eq!(format_failsafe_phase(3), "LANDED"); // FAILSAFE_LANDED
        assert_eq!(format_failsafe_phase(4), "RX_LOSS_MONITORING"); // FAILSAFE_RX_LOSS_MONITORING (new)
        assert_eq!(format_failsafe_phase(5), "RX_LOSS_RECOVERED"); // FAILSAFE_RX_LOSS_RECOVERED (new)
        assert_eq!(format_failsafe_phase(6), "GPS_RESCUE"); // FAILSAFE_GPS_RESCUE (new)

        // Test unknown phases (should return numeric string)
        assert_eq!(format_failsafe_phase(99), "99");
        assert_eq!(format_failsafe_phase(-1), "-1");
    }
}
