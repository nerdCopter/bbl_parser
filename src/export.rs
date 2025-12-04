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
    let base_name = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("blackbox");

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
pub fn export_to_gpx(
    input_path: &Path,
    log_index: usize,
    total_logs: usize,
    gps_coordinates: &[GpsCoordinate],
    _home_coordinates: &[GpsHomeCoordinate],
    export_options: &ExportOptions,
) -> Result<()> {
    if gps_coordinates.is_empty() {
        return Ok(());
    }

    let base_name = input_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let output_dir = export_options
        .output_dir
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| input_path.parent().unwrap_or(Path::new(".")));

    // Use consistent naming: only add suffix for multiple logs
    let log_suffix = if total_logs > 1 {
        format!(".{:02}", log_index + 1)
    } else {
        "".to_string()
    };
    let gpx_filename = output_dir.join(format!("{}{}.gps.gpx", base_name, log_suffix));

    let mut gpx_file = File::create(&gpx_filename)?;
    writeln!(gpx_file, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        gpx_file,
        r#"<gpx creator="BBL Parser (Rust)" version="1.1" xmlns="http://www.topografix.com/GPX/1/1" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd">"#
    )?;
    writeln!(
        gpx_file,
        "<metadata><name>Blackbox flight log</name></metadata>"
    )?;
    writeln!(gpx_file, "<trk><name>Blackbox flight log</name><trkseg>")?;

    for coord in gps_coordinates {
        // Only include coordinates with sufficient GPS satellite count (minimum 5)
        if let Some(num_sats) = coord.num_sats {
            if num_sats < 5 {
                continue;
            }
        }

        // Convert timestamp to ISO format
        let total_seconds = coord.timestamp_us / 1_000_000;
        let microseconds = coord.timestamp_us % 1_000_000;

        // Use March 26, 2025 as base date
        let hours = 5 + (total_seconds / 3600) % 24;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        writeln!(
            gpx_file,
            r#"  <trkpt lat="{:.7}" lon="{:.7}"><ele>{:.2}</ele><time>2025-03-26T{:02}:{:02}:{:02}.{:06}Z</time></trkpt>"#,
            coord.latitude, coord.longitude, coord.altitude, hours, minutes, seconds, microseconds
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

    let base_name = input_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let output_dir = export_options
        .output_dir
        .as_deref()
        .map(Path::new)
        .unwrap_or_else(|| input_path.parent().unwrap_or(Path::new(".")));

    // Use consistent naming: only add suffix for multiple logs
    let log_suffix = if total_logs > 1 {
        format!(".{:02}", log_index + 1)
    } else {
        "".to_string()
    };
    let event_filename = output_dir.join(format!("{}{}.event", base_name, log_suffix));

    let mut event_file = File::create(&event_filename)?;

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
