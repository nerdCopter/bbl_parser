use crate::types::*;
use crate::Result;
use anyhow::{anyhow, Context};
use std::path::Path;

/// Parse BBL file and return all logs (for CLI and multi-log processing)
pub fn parse_bbl_file_all_logs(
    file_path: &Path,
    export_options: crate::ExportOptions,
    debug: bool,
) -> Result<Vec<BBLLog>> {
    if debug {
        println!("=== PARSING BBL FILE ===");
        let metadata = std::fs::metadata(file_path)?;
        println!(
            "File size: {} bytes ({:.2} MB)",
            metadata.len(),
            metadata.len() as f64 / 1024.0 / 1024.0
        );
    }

    let file_data = std::fs::read(file_path)
        .with_context(|| format!("Failed to read BBL file: {:?}", file_path))?;

    parse_bbl_bytes_all_logs(&file_data, export_options, debug)
}

/// Parse BBL file and return first log (for library API compatibility)
pub fn parse_bbl_file(
    file_path: &Path,
    export_options: crate::ExportOptions,
    debug: bool,
) -> Result<BBLLog> {
    let logs = parse_bbl_file_all_logs(file_path, export_options, debug)?;
    logs.into_iter()
        .next()
        .ok_or_else(|| anyhow!("No logs found in BBL file"))
}

/// Parse BBL data from memory and return all logs
pub fn parse_bbl_bytes_all_logs(
    data: &[u8],
    _export_options: crate::ExportOptions,
    debug: bool,
) -> Result<Vec<BBLLog>> {
    if debug {
        println!("=== PARSING BBL DATA ===");
        println!("Data size: {} bytes", data.len());
    }

    // Look for multiple logs by searching for log start markers
    let log_start_marker = b"H Product:Blackbox flight data recorder by Nicholas Sherlock";
    let mut log_positions = Vec::new();

    // Find all log start positions
    for i in 0..data.len() {
        if i + log_start_marker.len() <= data.len()
            && &data[i..i + log_start_marker.len()] == log_start_marker
        {
            log_positions.push(i);
        }
    }

    if log_positions.is_empty() {
        return Err(anyhow!("No blackbox log headers found in data"));
    }

    if debug {
        println!("Found {} log(s) in data", log_positions.len());
    }

    // Parse all logs
    let mut logs = Vec::new();
    for (log_index, &start_pos) in log_positions.iter().enumerate() {
        if debug {
            println!(
                "Parsing log {} of {} (starting at position {})",
                log_index + 1,
                log_positions.len(),
                start_pos
            );
        }

        let end_pos = log_positions
            .get(log_index + 1)
            .copied()
            .unwrap_or(data.len());
        let log_data = &data[start_pos..end_pos];

        let log = parse_single_log(log_data, log_index + 1, log_positions.len(), debug)?;
        logs.push(log);
    }

    Ok(logs)
}

/// Parse BBL data from memory (returns first log for library API compatibility)
pub fn parse_bbl_bytes(
    data: &[u8],
    export_options: crate::ExportOptions,
    debug: bool,
) -> Result<BBLLog> {
    let logs = parse_bbl_bytes_all_logs(data, export_options, debug)?;
    logs.into_iter()
        .next()
        .ok_or_else(|| anyhow!("No logs found in BBL data"))
}

// Note: The rest of the parsing functions will be migrated from src/main.rs
// This is a placeholder for the systematic migration process

/// Internal function to parse a single BBL log from binary data
fn parse_single_log(
    log_data: &[u8],
    log_number: usize,
    total_logs: usize,
    debug: bool,
) -> Result<BBLLog> {
    // Find where headers end and binary data begins
    let mut header_end = 0;
    for i in 1..log_data.len() {
        if log_data[i - 1] == b'\n' && log_data[i] != b'H' {
            header_end = i;
            break;
        }
    }

    if header_end == 0 {
        header_end = log_data.len();
    }

    // Parse headers from the text section
    let header_text = std::str::from_utf8(&log_data[0..header_end])?;
    let header = crate::parser::header::parse_headers_from_text(header_text, debug)?;

    // Parse binary frame data
    let binary_data = &log_data[header_end..];
    let (mut stats, sample_frames, debug_frames, gps_coordinates, home_coordinates, event_frames) =
        crate::parser::frame::parse_frames(binary_data, &header, debug)?;

    // Update frame stats timing from actual frame data
    if !sample_frames.is_empty() {
        stats.start_time_us = sample_frames.first().unwrap().timestamp_us;
        stats.end_time_us = sample_frames.last().unwrap().timestamp_us;
    }

    let log = BBLLog {
        log_number,
        total_logs,
        header,
        stats,
        sample_frames,
        debug_frames,
        gps_coordinates,
        home_coordinates,
        event_frames,
    };

    Ok(log)
}
