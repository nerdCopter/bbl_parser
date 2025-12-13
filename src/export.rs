//! Export functionality for BBL data
//!
//! Contains functions for exporting parsed BBL data to various formats
//! including CSV, GPX, and Event files.

use crate::conversion::*;
use crate::types::*;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Export options for various output formats
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExportOptions {
    pub csv: bool,
    pub gpx: bool,
    pub event: bool,
    pub output_dir: Option<String>,
    pub force_export: bool,
}

/// Extract the base filename from an input path with consistent fallback.
/// Used by all export functions and path computation helpers to ensure
/// consistent naming across CSV, GPX, and event exports.
///
/// Always returns "blackbox" as fallback for missing or non-UTF-8 filenames,
/// ensuring compute_export_paths() predictions match actual export filenames.
fn extract_base_name(input_path: &Path) -> &str {
    input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("blackbox")
}

/// Helper to compute export file paths with consistent naming across all export types.
/// Ensures CLI status messages match actual filenames written by export functions.
///
/// # Arguments
/// * `input_path` - Path to the input BBL file (used to extract base filename)
/// * `export_options` - Export configuration with optional output directory
/// * `log_number` - 1-based log number (for .NN suffix when multiple logs)
/// * `total_logs` - Total number of logs in the file
///
/// # Returns
/// Tuple of (csv_path, headers_path, gpx_path, event_path) using consistent naming
pub fn compute_export_paths(
    input_path: &Path,
    export_options: &ExportOptions,
    log_number: usize,
    total_logs: usize,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let base_name = extract_base_name(input_path);

    let output_dir = if let Some(ref dir) = export_options.output_dir {
        std::path::Path::new(dir)
    } else {
        input_path.parent().unwrap_or(std::path::Path::new("."))
    };

    let log_suffix = if total_logs > 1 {
        format!(".{:02}", log_number)
    } else {
        String::new()
    };

    let csv_path = output_dir.join(format!("{}{}.csv", base_name, log_suffix));
    let headers_path = output_dir.join(format!("{}{}.headers.csv", base_name, log_suffix));
    let gpx_path = output_dir.join(format!("{}{}.gps.gpx", base_name, log_suffix));
    let event_path = output_dir.join(format!("{}{}.event", base_name, log_suffix));

    (csv_path, headers_path, gpx_path, event_path)
}

/// Pre-computed CSV field mapping for performance
#[derive(Debug)]
struct CsvFieldMap {
    field_name_to_lookup: Vec<(String, String)>, // (csv_name, lookup_name)
}

impl CsvFieldMap {
    fn new(header: &BBLHeader) -> Self {
        let mut field_name_to_lookup = Vec::new();

        // I frame fields
        for field_name in &header.i_frame_def.field_names {
            let trimmed = field_name.trim();
            let csv_name = if trimmed == "time" {
                "time (us)".to_string()
            } else if trimmed == "vbatLatest" {
                "vbatLatest (V)".to_string()
            } else if trimmed == "amperageLatest" {
                "amperageLatest (A)".to_string()
            } else {
                trimmed.to_string()
            };

            field_name_to_lookup.push((csv_name.clone(), trimmed.to_string()));
        }

        // Add computed fields IMMEDIATELY after I frame fields (like blackbox_decode does)
        if field_name_to_lookup
            .iter()
            .any(|(_, lookup)| lookup == "amperageLatest")
        {
            field_name_to_lookup.push(("energyCumulative (mAh)".to_string(), "".to_string()));
        }

        // S frame fields (with flag formatting)
        for field_name in &header.s_frame_def.field_names {
            let trimmed = field_name.trim();
            if trimmed == "time" {
                continue;
            } // Skip duplicate

            let csv_name = if trimmed.contains("Flag") || trimmed == "failsafePhase" {
                format!("{trimmed} (flags)")
            } else {
                trimmed.to_string()
            };

            field_name_to_lookup.push((csv_name.clone(), trimmed.to_string()));
        }

        Self {
            field_name_to_lookup,
        }
    }
}

/// Export BBL log to CSV format
pub fn export_to_csv(
    log: &BBLLog,
    input_path: &Path,
    export_options: &ExportOptions,
) -> Result<()> {
    let base_name = extract_base_name(input_path);

    let output_dir = if let Some(ref dir) = export_options.output_dir {
        Path::new(dir)
    } else {
        input_path.parent().unwrap_or(Path::new("."))
    };

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)?;
    }

    let log_suffix = if log.total_logs > 1 {
        format!(".{:02}", log.log_number)
    } else {
        "".to_string()
    };

    // Export plaintext headers to separate CSV
    let header_csv_path = output_dir.join(format!("{base_name}{log_suffix}.headers.csv"));
    export_headers_to_csv(&log.header, &header_csv_path)?;

    // Export flight data (I, P, S frames) to main CSV
    let flight_csv_path = output_dir.join(format!("{base_name}{log_suffix}.csv"));
    export_flight_data_to_csv(log, &flight_csv_path)?;

    Ok(())
}

/// Export headers to CSV file
fn export_headers_to_csv(header: &BBLHeader, output_path: &Path) -> Result<()> {
    let file = File::create(output_path)
        .with_context(|| format!("Failed to create headers CSV file: {output_path:?}"))?;
    let mut writer = BufWriter::new(file);

    // Write CSV header
    writeln!(writer, "Field,Value")?;

    // Parse and write all header lines
    for header_line in &header.all_headers {
        if let Some(content) = header_line.strip_prefix("H ") {
            // Remove "H " prefix and find the colon separator
            if let Some(colon_pos) = content.find(':') {
                let field_name = content[..colon_pos].trim();
                let field_value = content[colon_pos + 1..].trim();

                // Escape commas in values by wrapping in quotes
                let escaped_value = if field_value.contains(',') {
                    format!("\"{}\"", field_value.replace('"', "\"\""))
                } else {
                    field_value.to_string()
                };

                writeln!(writer, "{field_name},{escaped_value}")?;
            }
        }
    }

    writer
        .flush()
        .with_context(|| format!("Failed to flush headers CSV file: {output_path:?}"))?;

    Ok(())
}

/// Export flight data to CSV file
fn export_flight_data_to_csv(log: &BBLLog, output_path: &Path) -> Result<()> {
    let file = File::create(output_path)
        .with_context(|| format!("Failed to create flight data CSV file: {output_path:?}"))?;
    let mut writer = BufWriter::new(file);

    // Build optimized field mapping
    let csv_map = CsvFieldMap::new(&log.header);
    let field_names: Vec<String> = csv_map
        .field_name_to_lookup
        .iter()
        .map(|(csv_name, _)| csv_name.clone())
        .collect();

    // Collect all I and P frames in chronological order
    let mut all_frames: Vec<(u64, char, &DecodedFrame)> = Vec::new();

    // Use log.frames which contains all parsed frames
    for frame in &log.frames {
        if frame.frame_type == 'I' || frame.frame_type == 'P' {
            all_frames.push((frame.timestamp_us, frame.frame_type, frame));
        }
    }

    // Sort by timestamp
    all_frames.sort_by_key(|(timestamp, _, _)| *timestamp);

    if all_frames.is_empty() {
        return Ok(()); // No data to export
    }

    // Write field names header
    for (i, field_name) in field_names.iter().enumerate() {
        if i > 0 {
            write!(writer, ", ")?;
        }
        write!(writer, "{field_name}")?;
    }
    writeln!(writer)?;

    // Optimized CSV writing with pre-computed mappings
    let mut cumulative_energy_mah = 0f32;
    let mut last_timestamp_us = 0u64;
    let mut latest_s_frame_data: HashMap<String, i32> = HashMap::new();

    for (output_iteration, (timestamp, frame_type, frame)) in all_frames.iter().enumerate() {
        // Update latest S-frame data if this is an S frame
        if *frame_type == 'S' {
            for (key, value) in &frame.data {
                latest_s_frame_data.insert(key.clone(), *value);
            }
        }

        // Calculate energyCumulative for this frame
        if let Some(current_raw) = frame.data.get("amperageLatest").copied() {
            if last_timestamp_us > 0 && *timestamp > last_timestamp_us {
                let time_delta_hours = (*timestamp - last_timestamp_us) as f32 / 3_600_000_000.0;
                let current_amps = convert_amperage_to_amps(current_raw);
                cumulative_energy_mah += current_amps * time_delta_hours * 1000.0;
            }
            last_timestamp_us = *timestamp;
        }

        // Write data row using optimized field mapping
        for (i, (csv_name, lookup_name)) in csv_map.field_name_to_lookup.iter().enumerate() {
            if i > 0 {
                write!(writer, ", ")?;
            }

            // Fast path for special fields using pre-computed indices
            if csv_name == "time (us)" {
                write!(writer, "{}", *timestamp as i32)?;
            } else if csv_name == "loopIteration" {
                let value = frame
                    .data
                    .get("loopIteration")
                    .copied()
                    .unwrap_or(output_iteration as i32);
                write!(writer, "{value:4}")?;
            } else if csv_name == "vbatLatest (V)" {
                let raw_value = frame.data.get("vbatLatest").copied().unwrap_or(0);
                write!(
                    writer,
                    "{:4.1}",
                    convert_vbat_to_volts(raw_value, &log.header.firmware_revision)
                )?;
            } else if csv_name == "amperageLatest (A)" {
                let raw_value = frame.data.get("amperageLatest").copied().unwrap_or(0);
                write!(writer, "{:4.2}", convert_amperage_to_amps(raw_value))?;
            } else if csv_name == "energyCumulative (mAh)" {
                write!(writer, "{:5}", cumulative_energy_mah as i32)?;
            } else if csv_name.ends_with(" (flags)") {
                // Handle flag fields - output text values like blackbox_decode.c
                let raw_value = frame
                    .data
                    .get(lookup_name)
                    .copied()
                    .or_else(|| latest_s_frame_data.get(lookup_name).copied())
                    .unwrap_or(0);

                let formatted = if lookup_name == "flightModeFlags" {
                    format_flight_mode_flags(raw_value)
                } else if lookup_name == "stateFlags" {
                    format_state_flags(raw_value)
                } else if lookup_name == "failsafePhase" {
                    format_failsafe_phase(raw_value)
                } else {
                    raw_value.to_string()
                };
                write!(writer, "{formatted}")?;
            } else {
                // Regular field lookup with S-frame fallback
                let value = frame
                    .data
                    .get(lookup_name)
                    .copied()
                    .or_else(|| latest_s_frame_data.get(lookup_name).copied())
                    .unwrap_or(0);
                write!(writer, "{value:4}")?;
            }
        }
        writeln!(writer)?;
    }

    writer
        .flush()
        .with_context(|| format!("Failed to flush flight data CSV file: {output_path:?}"))?;

    Ok(())
}

/// Export GPS data to GPX format
///
/// # Arguments
/// * `input_path` - Path to the input BBL file (used for output naming)
/// * `log_index` - Index of the current log (0-based)
/// * `total_logs` - Total number of logs in the file
/// * `gps_coordinates` - GPS coordinate data to export
/// * `home_coordinates` - Home coordinates from H frames (used for home waypoint marker)
/// * `export_options` - Export configuration options
/// * `log_start_datetime` - Optional log start datetime from header for accurate timestamps
///
/// # Features
/// When home coordinates are available, adds a home position waypoint to the GPX file.
/// This provides a visual reference point in GPS mapping tools.
///
/// # Performance Notes
/// For very large GPS traces, the `log_start_datetime` is parsed via `generate_gpx_timestamp()`
/// on each trackpoint. Future optimization: consider caching the parsed base epoch once per log
/// to avoid repeated parsing overhead when exporting thousands of GPS points.
pub fn export_to_gpx(
    input_path: &Path,
    log_index: usize,
    total_logs: usize,
    gps_coordinates: &[GpsCoordinate],
    home_coordinates: &[GpsHomeCoordinate],
    export_options: &ExportOptions,
    log_start_datetime: Option<&str>,
) -> Result<()> {
    if gps_coordinates.is_empty() {
        return Ok(());
    }

    // Use compute_export_paths to ensure consistent naming with CSV exports
    let (_, _, gpx_path, _) =
        compute_export_paths(input_path, export_options, log_index + 1, total_logs);

    // Create output directory if it doesn't exist (match export_to_csv behavior)
    if let Some(parent) = gpx_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut gpx_file = File::create(&gpx_path)?;
    writeln!(gpx_file, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        gpx_file,
        r#"<gpx creator="BBL Parser (Rust)" version="1.1" xmlns="http://www.topografix.com/GPX/1/1" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd">"#
    )?;
    writeln!(
        gpx_file,
        "<metadata><name>Blackbox flight log</name></metadata>"
    )?;

    // Add home position waypoint if available
    if let Some(home) = home_coordinates.first() {
        writeln!(
            gpx_file,
            r#"  <wpt lat="{:.7}" lon="{:.7}">"#,
            home.home_latitude, home.home_longitude
        )?;
        writeln!(gpx_file, r#"    <name>Home</name>"#)?;
        writeln!(gpx_file, r#"    <sym>Flag</sym>"#)?;
        writeln!(gpx_file, r#"    <desc>Home Position</desc>"#)?;
        writeln!(gpx_file, r#"  </wpt>"#)?;
    }

    writeln!(gpx_file, "<trk><name>Blackbox flight log</name><trkseg>")?;

    for coord in gps_coordinates {
        // Only include coordinates with sufficient GPS satellite count (minimum 5)
        if let Some(num_sats) = coord.num_sats {
            if num_sats < 5 {
                continue;
            }
        }

        // Generate GPX timestamp from log_start_datetime + frame timestamp
        // Following blackbox_decode approach: dateTime + (gpsFrameTime / 1000000)
        let timestamp_str = generate_gpx_timestamp(log_start_datetime, coord.timestamp_us);

        writeln!(
            gpx_file,
            r#"  <trkpt lat="{:.7}" lon="{:.7}"><ele>{:.2}</ele><time>{}</time></trkpt>"#,
            coord.latitude, coord.longitude, coord.altitude, timestamp_str
        )?;
    }

    writeln!(gpx_file, "</trkseg></trk>")?;
    writeln!(gpx_file, "</gpx>")?;

    Ok(())
}

/// Export event data to file
pub fn export_to_event(
    input_path: &Path,
    log_index: usize,
    total_logs: usize,
    event_frames: &[EventFrame],
    export_options: &ExportOptions,
) -> Result<()> {
    if event_frames.is_empty() {
        return Ok(());
    }

    // Use compute_export_paths to ensure consistent naming with CSV exports
    let (_, _, _, event_path) =
        compute_export_paths(input_path, export_options, log_index + 1, total_logs);

    // Create output directory if it doesn't exist (match export_to_csv behavior)
    if let Some(parent) = event_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut event_file = File::create(&event_path)?;

    // Export as JSONL format (individual JSON objects per line) to match blackbox_decode
    for event in event_frames.iter() {
        writeln!(
            event_file,
            r#"{{"name":"{}", "time":{}}}"#,
            event.event_name.replace('"', "\\\""),
            event.timestamp_us
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::TempDir;

    /// Test helper to create a minimal GPX export and read back the content
    fn export_gpx_and_read(
        gps_coords: &[GpsCoordinate],
        home_coords: &[GpsHomeCoordinate],
    ) -> Result<String> {
        let temp_dir = TempDir::new()?;
        let temp_input_path = temp_dir.path().join("test_input.bbl");

        let export_opts = ExportOptions {
            csv: false,
            gpx: true,
            event: false,
            output_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
            force_export: false,
        };

        export_to_gpx(
            &temp_input_path,
            0,
            1,
            gps_coords,
            home_coords,
            &export_opts,
            None,
        )?;

        // Read back the generated GPX file
        let gpx_path = temp_dir.path().join("test_input.gps.gpx");
        let mut gpx_content = String::new();
        let mut gpx_file = File::open(&gpx_path)?;
        gpx_file.read_to_string(&mut gpx_content)?;

        Ok(gpx_content)
    }

    #[test]
    fn test_gpx_home_waypoint_with_coordinates() -> Result<()> {
        let home_coords = vec![GpsHomeCoordinate {
            home_latitude: 40.7128,
            home_longitude: -74.0060,
            timestamp_us: 0,
        }];

        let gps_coords = vec![GpsCoordinate {
            latitude: 40.7129,
            longitude: -74.0061,
            altitude: 100.0,
            timestamp_us: 1_000_000,
            num_sats: Some(10),
            speed: Some(5.0),
            ground_course: Some(180.0),
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify home waypoint element exists
        assert!(
            content.contains(r#"<wpt lat="40.7128000" lon="-74.0060000">"#),
            "Home waypoint coordinates should be formatted to 7 decimal places"
        );

        // Verify home waypoint has correct structure
        assert!(
            content.contains("<name>Home</name>"),
            "Home waypoint should have <name>Home</name>"
        );
        assert!(
            content.contains("<sym>Flag</sym>"),
            "Home waypoint should have <sym>Flag</sym>"
        );
        assert!(
            content.contains("<desc>Home Position</desc>"),
            "Home waypoint should have <desc>Home Position</desc>"
        );

        // Verify closing tag
        assert!(
            content.contains("</wpt>"),
            "Home waypoint should have closing tag"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_home_waypoint_precision() -> Result<()> {
        let home_coords = vec![GpsHomeCoordinate {
            home_latitude: 51.5074123456789,
            home_longitude: -0.1278123456789,
            timestamp_us: 0,
        }];

        let gps_coords = vec![GpsCoordinate {
            latitude: 51.5075,
            longitude: -0.1280,
            altitude: 50.0,
            timestamp_us: 1_000_000,
            num_sats: Some(5),
            speed: None,
            ground_course: None,
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify coordinates are truncated/rounded to 7 decimal places
        assert!(
            content.contains(r#"lat="51.5074123""#),
            "Latitude should be formatted to 7 decimal places (truncated)"
        );
        assert!(
            content.contains(r#"lon="-0.1278123""#),
            "Longitude should be formatted to 7 decimal places (truncated)"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_no_home_waypoint_when_empty() -> Result<()> {
        let home_coords = vec![];

        let gps_coords = vec![GpsCoordinate {
            latitude: 40.7129,
            longitude: -74.0061,
            altitude: 100.0,
            timestamp_us: 1_000_000,
            num_sats: Some(10),
            speed: Some(5.0),
            ground_course: Some(180.0),
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify no home waypoint appears when home_coordinates is empty
        assert!(
            !content.contains("<name>Home</name>"),
            "No home waypoint should be present when home_coordinates is empty"
        );
        assert!(
            !content.contains("<wpt"),
            "No waypoint element should be present when home_coordinates is empty"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_home_waypoint_xml_structure() -> Result<()> {
        let home_coords = vec![GpsHomeCoordinate {
            home_latitude: 35.6762,
            home_longitude: 139.6503,
            timestamp_us: 0,
        }];

        let gps_coords = vec![GpsCoordinate {
            latitude: 35.6763,
            longitude: 139.6504,
            altitude: 25.0,
            timestamp_us: 1_000_000,
            num_sats: Some(8),
            speed: Some(2.0),
            ground_course: Some(45.0),
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify basic GPX structure
        assert!(
            content.contains(r#"<?xml version="1.0" encoding="UTF-8"?>"#),
            "XML declaration should be present"
        );
        assert!(
            content.contains("version=\"1.1\""),
            "GPX version should be 1.1"
        );
        assert!(
            content.contains("xmlns=\"http://www.topografix.com/GPX/1/1\""),
            "GPX namespace should be present"
        );

        // Verify home waypoint appears before track element
        let wpt_index = content.find("<wpt").expect("Should have <wpt element");
        let trk_index = content.find("<trk").expect("Should have <trk element");
        assert!(
            wpt_index < trk_index,
            "Home waypoint should appear before track element in GPX file"
        );

        // Verify proper nesting and indentation
        assert!(
            content.contains("  <wpt"),
            "Home waypoint should be properly indented"
        );
        assert!(
            content.contains("    <name>Home</name>"),
            "Home waypoint child elements should be properly indented"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_home_waypoint_with_negative_coordinates() -> Result<()> {
        let home_coords = vec![GpsHomeCoordinate {
            home_latitude: -33.8688,
            home_longitude: 151.2093,
            timestamp_us: 0,
        }];

        let gps_coords = vec![GpsCoordinate {
            latitude: -33.8689,
            longitude: 151.2094,
            altitude: 150.0,
            timestamp_us: 1_000_000,
            num_sats: Some(12),
            speed: Some(10.0),
            ground_course: Some(270.0),
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify negative coordinates are correctly formatted
        assert!(
            content.contains(r#"lat="-33.8688000""#),
            "Negative latitude should be correctly formatted"
        );
        assert!(
            content.contains(r#"lon="151.2093000""#),
            "Positive longitude should be correctly formatted"
        );

        // Verify structure is still correct with negative values
        assert!(
            content.contains("<name>Home</name>"),
            "Structure should be correct"
        );
        assert!(content.contains("</wpt>"), "Closing tag should be present");

        Ok(())
    }

    #[test]
    fn test_gpx_only_first_home_coordinate_used() -> Result<()> {
        let home_coords = vec![
            GpsHomeCoordinate {
                home_latitude: 40.7128,
                home_longitude: -74.0060,
                timestamp_us: 0,
            },
            GpsHomeCoordinate {
                home_latitude: 51.5074,
                home_longitude: -0.1278,
                timestamp_us: 1_000_000,
            },
        ];

        let gps_coords = vec![GpsCoordinate {
            latitude: 40.7129,
            longitude: -74.0061,
            altitude: 100.0,
            timestamp_us: 1_000_000,
            num_sats: Some(10),
            speed: Some(5.0),
            ground_course: Some(180.0),
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify only first coordinate is used
        assert!(
            content.contains(r#"lat="40.7128000""#),
            "First home coordinate should be used"
        );
        // Count occurrences of <wpt to ensure only one home waypoint
        let wpt_count = content.matches("<wpt").count();
        assert_eq!(
            wpt_count, 1,
            "Only one home waypoint should be present when multiple home_coordinates exist"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_empty_gps_coordinates_returns_ok() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let temp_input_path = temp_dir.path().join("test_input.bbl");

        let export_opts = ExportOptions {
            csv: false,
            gpx: true,
            event: false,
            output_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
            force_export: false,
        };

        let home_coords = vec![GpsHomeCoordinate {
            home_latitude: 40.7128,
            home_longitude: -74.0060,
            timestamp_us: 0,
        }];

        // Should return Ok even with empty GPS coordinates
        let result = export_to_gpx(
            &temp_input_path,
            0,
            1,
            &[],
            &home_coords,
            &export_opts,
            None,
        );
        assert!(
            result.is_ok(),
            "Export should succeed with empty GPS coordinates"
        );

        // Verify no GPX file is created when GPS coordinates are empty
        let gpx_path = temp_dir.path().join("test_input.gps.gpx");
        assert!(
            !gpx_path.exists(),
            "No GPX file should be created when GPS coordinates are empty"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_trackpoints_skip_low_satellite_count() -> Result<()> {
        let home_coords = vec![];

        let gps_coords = vec![
            GpsCoordinate {
                latitude: 40.7129,
                longitude: -74.0061,
                altitude: 100.0,
                timestamp_us: 1_000_000,
                num_sats: Some(3), // Below minimum of 5
                speed: Some(5.0),
                ground_course: Some(180.0),
            },
            GpsCoordinate {
                latitude: 40.7130,
                longitude: -74.0062,
                altitude: 105.0,
                timestamp_us: 2_000_000,
                num_sats: Some(10), // Valid
                speed: Some(5.0),
                ground_course: Some(180.0),
            },
        ];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify that the trackpoint with low satellite count is not in output
        assert!(
            !content.contains("40.7129"),
            "Trackpoint with low satellite count should be excluded"
        );

        // Verify that the valid trackpoint is in output
        assert!(
            content.contains("40.7130"),
            "Trackpoint with sufficient satellites should be included"
        );

        Ok(())
    }
}
