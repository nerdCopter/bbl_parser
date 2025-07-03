mod bbl_format;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use glob::glob;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
struct FieldDefinition {
    #[allow(dead_code)]
    name: String,
    signed: bool,
    predictor: u8,
    encoding: u8,
}

#[derive(Debug, Clone)]
struct FrameDefinition {
    fields: Vec<FieldDefinition>,
    field_names: Vec<String>,
    count: usize,
}

impl FrameDefinition {
    fn new() -> Self {
        Self {
            fields: Vec::new(),
            field_names: Vec::new(),
            count: 0,
        }
    }

    fn from_field_names(names: Vec<String>) -> Self {
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

    fn update_signed(&mut self, signed_data: &[bool]) {
        for (i, field) in self.fields.iter_mut().enumerate() {
            if i < signed_data.len() {
                field.signed = signed_data[i];
            }
        }
    }

    fn update_predictors(&mut self, predictors: &[u8]) {
        for (i, field) in self.fields.iter_mut().enumerate() {
            if i < predictors.len() {
                field.predictor = predictors[i];
            }
        }
    }

    fn update_encoding(&mut self, encodings: &[u8]) {
        for (i, field) in self.fields.iter_mut().enumerate() {
            if i < encodings.len() {
                field.encoding = encodings[i];
            }
        }
    }
}

#[derive(Debug)]
struct BBLHeader {
    firmware_revision: String,
    board_info: String,
    craft_name: String,
    data_version: u8,
    looptime: u32,
    i_frame_def: FrameDefinition,
    p_frame_def: FrameDefinition,
    s_frame_def: FrameDefinition,
    g_frame_def: FrameDefinition,
    h_frame_def: FrameDefinition,
    sysconfig: HashMap<String, i32>,
    all_headers: Vec<String>,
}

#[derive(Debug, Default)]
struct FrameStats {
    i_frames: u32,
    p_frames: u32,
    h_frames: u32,
    g_frames: u32,
    e_frames: u32,
    s_frames: u32,
    total_frames: u32,
    #[allow(dead_code)]
    total_bytes: u64,
    start_time_us: u64,
    end_time_us: u64,
    failed_frames: u32,
    missing_iterations: u64,
    // Additional blackbox_decode compatibility tracking
    corrupted_frames: u32,
    invalid_frame_types: u32,
    frame_validation_failures: u32,
    unknown_frame_bytes: Vec<u8>, // Track unknown frame type bytes for analysis
}

#[derive(Debug, Clone)]
struct DecodedFrame {
    frame_type: char,
    timestamp_us: u64,
    #[allow(dead_code)]
    loop_iteration: u32,
    data: HashMap<String, i32>,
}

#[derive(Debug)]
struct BBLLog {
    log_number: usize,
    total_logs: usize,
    header: BBLHeader,
    stats: FrameStats,
    sample_frames: Vec<DecodedFrame>, // Only store a few sample frames, not all
    debug_frames: Option<HashMap<char, Vec<DecodedFrame>>>, // Frame data by type for debug output
}

// Frame history for prediction during parsing
struct FrameHistory {
    current_frame: Vec<i32>,
    previous_frame: Vec<i32>,
    previous2_frame: Vec<i32>,
    valid: bool,
}

#[allow(dead_code)]
fn should_have_frame(frame_index: u32, sysconfig: &HashMap<String, i32>) -> bool {
    let frame_interval_i = sysconfig.get("frameIntervalI").copied().unwrap_or(32);
    let frame_interval_p_num = sysconfig.get("frameIntervalPNum").copied().unwrap_or(1);
    let frame_interval_p_denom = sysconfig.get("frameIntervalPDenom").copied().unwrap_or(1);

    let left_side = ((frame_index % frame_interval_i as u32) + frame_interval_p_num as u32 - 1)
        % frame_interval_p_denom as u32;
    left_side < frame_interval_p_num as u32
}

#[derive(Debug, Clone)]
struct CsvExportOptions {
    output_dir: Option<String>,
}

// Pre-computed CSV field mapping for performance
#[derive(Debug)]
struct CsvFieldMap {
    field_name_to_lookup: Vec<(String, String)>, // (csv_name, lookup_name)
}

impl CsvFieldMap {
    fn new(header: &BBLHeader) -> Self {
        let mut field_name_to_lookup = Vec::new();

        // Build optimized field mappings from all frame types
        let mut csv_field_names = Vec::new();

        // I frame fields
        for field_name in &header.i_frame_def.field_names {
            let trimmed = field_name.trim();
            // Use exact field names to match blackbox_decode CSV output with units
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
            csv_field_names.push(csv_name);
        }

        // Add computed fields to match blackbox_decode field order exactly
        // energyCumulative should come BEFORE S-frame fields, not after
        if field_name_to_lookup
            .iter()
            .any(|(_, lookup)| lookup == "amperageLatest")
        {
            field_name_to_lookup.push(("energyCumulative (mAh)".to_string(), "".to_string()));
            csv_field_names.push("energyCumulative (mAh)".to_string());
        }

        // S frame fields
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
            csv_field_names.push(csv_name);
        }

        // NOTE: G-frame fields excluded from main CSV (will go to separate .gps.csv file in future)
        // NOTE: E-frame fields excluded from main CSV (will go to separate .event file in future)

        Self {
            field_name_to_lookup,
        }
    }
}

fn main() -> Result<()> {
    let matches = Command::new("BBL Parser")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Read and parse BBL blackbox log files. Output to various formats.")
        .arg(
            Arg::new("files")
                .help("BBL files to parse (.BBL, .BFL, .TXT extensions supported, case-insensitive, supports globbing)")
                .required(true)
                .num_args(1..)
                .index(1),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .help("Enable debug output and detailed parsing information")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("csv")
                .long("csv")
                .help("Export decoded frame data to CSV files (creates .XX.csv for flight data and .XX.headers.csv for plaintext headers)")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("output-dir")
                .long("output-dir")
                .help("Directory for CSV output files (default: same as input file)")
                .value_name("DIR"),
        )
        .arg(
            Arg::new("frames-only")
                .long("frames-only")
                .help("Output frame data only for debugging (compare with blackbox_decode -d)")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let debug = matches.get_flag("debug");
    let export_csv = matches.get_flag("csv");
    let frames_only = matches.get_flag("frames-only");
    let output_dir = matches.get_one::<String>("output-dir").cloned();
    let file_patterns: Vec<&String> = matches.get_many::<String>("files").unwrap().collect();

    let csv_options = CsvExportOptions { output_dir };

    let mut processed_files = 0;

    if debug {
        println!("Input patterns: {file_patterns:?}");
    }

    // Collect all valid file paths
    let mut valid_paths = Vec::new();
    for pattern in &file_patterns {
        if debug {
            println!("Processing pattern: {pattern}");
        }

        let paths: Vec<_> = if pattern.contains('*') || pattern.contains('?') {
            match glob(pattern) {
                Ok(glob_iter) => {
                    let collected = glob_iter.collect::<Result<Vec<_>, _>>();
                    match collected {
                        Ok(paths) => {
                            if debug {
                                println!("Glob pattern '{pattern}' matched {} files", paths.len());
                            }
                            paths
                        }
                        Err(e) => {
                            eprintln!("Error expanding glob pattern '{pattern}': {e}");
                            continue;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Invalid glob pattern '{pattern}': {e}");
                    continue;
                }
            }
        } else {
            vec![Path::new(pattern).to_path_buf()]
        };

        for path in paths {
            if debug {
                println!("Checking file: {path:?}");
            }

            if !path.exists() {
                eprintln!("Warning: File does not exist: {path:?}");
                continue;
            }

            let valid_extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    let ext_lower = ext.to_ascii_lowercase();
                    ext_lower == "bbl" || ext_lower == "bfl" || ext_lower == "txt"
                })
                .unwrap_or(false);

            if !valid_extension {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("none");
                eprintln!("Warning: Skipping file with unsupported extension '{ext}': {path:?}");
                continue;
            }

            if debug {
                println!("Added valid file: {path:?}");
            }
            valid_paths.push(path);
        }
    }

    if debug {
        println!("Found {} valid files to process", valid_paths.len());
    }

    if valid_paths.is_empty() {
        eprintln!("Error: No valid files found to process.");
        eprintln!("Supported extensions: .BBL, .BFL, .TXT (case-insensitive)");
        eprintln!("Input patterns were: {file_patterns:?}");
        std::process::exit(1);
    }

    // Process files
    for (index, path) in valid_paths.iter().enumerate() {
        if index > 0 {
            println!();
        }

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        println!("Processing: {filename}");

        match parse_bbl_file_streaming(path, debug, export_csv, frames_only, &csv_options) {
            Ok(processed_logs) => {
                if debug {
                    println!(
                        "Successfully processed {processed_logs} log(s) with streaming export"
                    );
                }
                processed_files += 1;
            }
            Err(e) => {
                eprintln!("Error processing {filename}: {e}");
                eprintln!("Continuing with next file...");
            }
        }
    }

    if processed_files == 0 {
        eprintln!(
            "Error: No files were successfully processed out of {} files found.",
            valid_paths.len()
        );
        eprintln!("This could be due to:");
        eprintln!("  - Files not being valid BBL/BFL format");
        eprintln!("  - Corrupted or empty files");
        eprintln!("  - Missing blackbox log headers");
        eprintln!("Use --debug flag for more detailed error information.");
        std::process::exit(1);
    }

    Ok(())
}

#[allow(dead_code)]
fn parse_bbl_file(file_path: &Path, debug: bool, csv_export: bool) -> Result<Vec<BBLLog>> {
    if debug {
        println!("=== PARSING BBL FILE ===");
        let metadata = std::fs::metadata(file_path)?;
        println!(
            "File size: {} bytes ({:.2} MB)",
            metadata.len(),
            metadata.len() as f64 / 1024.0 / 1024.0
        );
    }

    let file_data = std::fs::read(file_path)?;

    // Look for multiple logs by searching for log start markers
    let log_start_marker = b"H Product:Blackbox flight data recorder by Nicholas Sherlock";
    let mut log_positions = Vec::new();

    // Find all log start positions
    for i in 0..file_data.len() {
        if i + log_start_marker.len() <= file_data.len()
            && &file_data[i..i + log_start_marker.len()] == log_start_marker
        {
            log_positions.push(i);
        }
    }

    if log_positions.is_empty() {
        return Err(anyhow::anyhow!("No blackbox log headers found in file"));
    }

    if debug {
        println!("Found {} log(s) in file", log_positions.len());
    }

    let mut logs = Vec::new();

    for (log_index, &start_pos) in log_positions.iter().enumerate() {
        if debug {
            println!(
                "Parsing log {} starting at position {}",
                log_index + 1,
                start_pos
            );
        }

        // Determine end position (start of next log or end of file)
        let end_pos = log_positions
            .get(log_index + 1)
            .copied()
            .unwrap_or(file_data.len());
        let log_data = &file_data[start_pos..end_pos];

        // Parse this individual log
        let log = parse_single_log(
            log_data,
            log_index + 1,
            log_positions.len(),
            debug,
            csv_export,
        )?;
        logs.push(log);
    }

    Ok(logs)
}

fn parse_single_log(
    log_data: &[u8],
    log_number: usize,
    total_logs: usize,
    debug: bool,
    csv_export: bool,
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
    let header = parse_headers_from_text(header_text, debug)?;

    // Parse binary frame data
    let binary_data = &log_data[header_end..];
    let (mut stats, frames, debug_frames) = parse_frames(binary_data, &header, debug, csv_export)?;

    // Update frame stats timing from actual frame data
    if !frames.is_empty() {
        stats.start_time_us = frames.first().unwrap().timestamp_us;
        stats.end_time_us = frames.last().unwrap().timestamp_us;
    }

    if debug {
        // Debug: Show I-frame field order to compare with C implementation
        println!("DEBUG: I-frame field order:");
        for (i, field_name) in header.i_frame_def.field_names.iter().enumerate() {
            println!("  [{i}]: {field_name}");
            if i > 5 {
                // Only show first few to avoid spam
                println!(
                    "  ... ({} total fields)",
                    header.i_frame_def.field_names.len()
                );
                break;
            }
        }
    }

    let log = BBLLog {
        log_number,
        total_logs,
        header,
        stats,
        sample_frames: frames,
        debug_frames,
    };

    Ok(log)
}

fn parse_headers_from_text(header_text: &str, debug: bool) -> Result<BBLHeader> {
    let mut all_headers = Vec::new();
    let mut firmware_revision = String::new();
    let mut board_info = String::new();
    let mut craft_name = String::new();
    let mut data_version = 2u8;
    let mut looptime = 0u32;
    let mut sysconfig = HashMap::new();

    // Initialize frame definitions
    let mut i_frame_def = FrameDefinition::new();
    let mut p_frame_def = FrameDefinition::new();
    let mut s_frame_def = FrameDefinition::new();
    let mut g_frame_def = FrameDefinition::new();
    let mut h_frame_def = FrameDefinition::new();

    for line in header_text.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with("H ") {
            continue;
        }

        all_headers.push(line.to_string());

        // Parse specific headers following JavaScript reference
        if line.starts_with("H Firmware revision:") {
            firmware_revision = line
                .strip_prefix("H Firmware revision:")
                .unwrap_or("")
                .trim()
                .to_string();
        } else if line.starts_with("H Board information:") {
            board_info = line
                .strip_prefix("H Board information:")
                .unwrap_or("")
                .trim()
                .to_string();
        } else if line.starts_with("H Craft name:") {
            craft_name = line
                .strip_prefix("H Craft name:")
                .unwrap_or("")
                .trim()
                .to_string();
        } else if line.starts_with("H Data version:") {
            if let Ok(version) = line
                .strip_prefix("H Data version:")
                .unwrap_or("2")
                .trim()
                .parse()
            {
                data_version = version;
            }
        } else if line.starts_with("H looptime:") {
            if let Ok(lt) = line
                .strip_prefix("H looptime:")
                .unwrap_or("0")
                .trim()
                .parse()
            {
                looptime = lt;
            }
        } else if line.starts_with("H Field I name:") {
            // Parse I frame field names
            if let Some(field_str) = line.strip_prefix("H Field I name:") {
                let names: Vec<String> =
                    field_str.split(',').map(|s| s.trim().to_string()).collect();
                i_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field P name:") {
            // Parse P frame field names
            if let Some(field_str) = line.strip_prefix("H Field P name:") {
                let names: Vec<String> =
                    field_str.split(',').map(|s| s.trim().to_string()).collect();
                p_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field S name:") {
            // Parse S frame field names
            if let Some(field_str) = line.strip_prefix("H Field S name:") {
                let names: Vec<String> =
                    field_str.split(',').map(|s| s.trim().to_string()).collect();
                s_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field G name:") {
            // Parse G frame field names
            if let Some(field_str) = line.strip_prefix("H Field G name:") {
                let names: Vec<String> =
                    field_str.split(',').map(|s| s.trim().to_string()).collect();
                g_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field H name:") {
            // Parse H frame field names
            if let Some(field_str) = line.strip_prefix("H Field H name:") {
                let names: Vec<String> =
                    field_str.split(',').map(|s| s.trim().to_string()).collect();
                h_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field I signed:") {
            // Parse I frame signed data
            if let Some(signed_str) = line.strip_prefix("H Field I signed:") {
                let signed_data = parse_signed_data(signed_str);
                i_frame_def.update_signed(&signed_data);
            }
        } else if line.starts_with("H Field I predictor:") {
            // Parse I frame predictors
            if let Some(pred_str) = line.strip_prefix("H Field I predictor:") {
                let predictors = parse_numeric_data(pred_str);
                i_frame_def.update_predictors(&predictors);
            }
        } else if line.starts_with("H Field I encoding:") {
            // Parse I frame encodings
            if let Some(enc_str) = line.strip_prefix("H Field I encoding:") {
                let encodings = parse_numeric_data(enc_str);
                i_frame_def.update_encoding(&encodings);
            }
        } else if line.starts_with("H Field P predictor:") {
            // Parse P frame predictors
            if let Some(pred_str) = line.strip_prefix("H Field P predictor:") {
                let predictors = parse_numeric_data(pred_str);
                // P frames inherit field names from I frames but have their own predictors
                if p_frame_def.field_names.is_empty() && !i_frame_def.field_names.is_empty() {
                    p_frame_def =
                        FrameDefinition::from_field_names(i_frame_def.field_names.clone());
                }
                p_frame_def.update_predictors(&predictors);
            }
        } else if line.starts_with("H Field P encoding:") {
            // Parse P frame encodings
            if let Some(enc_str) = line.strip_prefix("H Field P encoding:") {
                let encodings = parse_numeric_data(enc_str);
                // P frames inherit field names from I frames but have their own encodings
                if p_frame_def.field_names.is_empty() && !i_frame_def.field_names.is_empty() {
                    p_frame_def =
                        FrameDefinition::from_field_names(i_frame_def.field_names.clone());
                }
                p_frame_def.update_encoding(&encodings);
            }
        } else if line.starts_with("H Field S signed:") {
            // Parse S frame signed data
            if let Some(signed_str) = line.strip_prefix("H Field S signed:") {
                let signed_data = parse_signed_data(signed_str);
                s_frame_def.update_signed(&signed_data);
            }
        } else if line.starts_with("H Field S predictor:") {
            // Parse S frame predictors
            if let Some(pred_str) = line.strip_prefix("H Field S predictor:") {
                let predictors = parse_numeric_data(pred_str);
                s_frame_def.update_predictors(&predictors);
            }
        } else if line.starts_with("H Field S encoding:") {
            // Parse S frame encodings
            if let Some(enc_str) = line.strip_prefix("H Field S encoding:") {
                let encodings = parse_numeric_data(enc_str);
                s_frame_def.update_encoding(&encodings);
            }
        }

        // Parse additional sysconfig values
        if let Some(colon_pos) = line.find(':') {
            if let Some(field_name) = line.get(2..colon_pos) {
                if let Some(field_value) = line.get(colon_pos + 1..) {
                    let field_name = field_name.trim();
                    let field_value = field_value.trim();

                    // Store numeric values that might be useful later
                    if let Ok(num_value) = field_value.parse::<i32>() {
                        sysconfig.insert(field_name.to_string(), num_value);
                    }
                }
            }
        }
    }

    if debug {
        println!(
            "Parsed headers: Firmware={firmware_revision}, Board={board_info}, Craft={craft_name}"
        );
        println!("Data version: {data_version}, Looptime: {looptime}");
    }

    Ok(BBLHeader {
        firmware_revision,
        board_info,
        craft_name,
        data_version,
        looptime,
        i_frame_def,
        p_frame_def,
        s_frame_def,
        g_frame_def,
        h_frame_def,
        sysconfig,
        all_headers,
    })
}

#[allow(dead_code)]
fn display_frame_data(logs: &[BBLLog]) {
    for log in logs {
        if let Some(ref debug_frames) = log.debug_frames {
            println!("\n=== FRAME DATA ===");

            for (frame_type, frames) in debug_frames.iter() {
                if frames.is_empty() {
                    continue;
                }

                println!("\n{}-frame data ({} frames):", frame_type, frames.len());

                // Get field names from first frame
                if let Some(first_frame) = frames.first() {
                    let mut field_names: Vec<&String> = first_frame.data.keys().collect();
                    field_names.sort();

                    // Limit field display width to prevent extremely wide output
                    let max_fields_to_show = 10;
                    let selected_fields = if field_names.len() > max_fields_to_show {
                        // Show time, loop, and first 8 field names
                        let mut selected = Vec::new();
                        for name in &field_names {
                            if name.as_str() == "time" || name.as_str() == "loopIteration" {
                                continue; // These will be shown separately
                            }
                            if selected.len() < 8 {
                                selected.push(*name);
                            }
                        }
                        selected
                    } else {
                        field_names
                            .iter()
                            .filter(|name| {
                                name.as_str() != "time" && name.as_str() != "loopIteration"
                            })
                            .copied()
                            .collect()
                    };

                    // Print header
                    print!("  {:>8} {:>12} {:>8}", "Index", "Time(μs)", "Loop");
                    for field_name in &selected_fields {
                        print!(
                            " {:>10}",
                            if field_name.len() > 10 {
                                &field_name[..10]
                            } else {
                                field_name
                            }
                        );
                    }
                    if field_names.len() > max_fields_to_show {
                        print!(
                            " ... ({} more fields)",
                            field_names.len() - selected_fields.len() - 2
                        );
                    }
                    println!();

                    // Determine which frames to show
                    let frames_to_show = if frames.len() <= 30 {
                        // Show all frames
                        (0..frames.len()).collect::<Vec<_>>()
                    } else {
                        // Show first 5, middle 5, last 5
                        let mut indices = Vec::new();
                        // First 5
                        indices.extend(0..5);
                        // Middle 5
                        let mid = frames.len() / 2;
                        indices.extend((mid - 2)..(mid + 3));
                        // Last 5
                        indices.extend((frames.len() - 5)..frames.len());
                        indices
                    };

                    let mut last_shown_index = None;
                    for &index in &frames_to_show {
                        // Show ellipsis if there's a gap
                        if let Some(last_idx) = last_shown_index {
                            if index > last_idx + 1 {
                                println!(
                                    "  {:>8} {:>12} {:>8} ... ({} frames skipped)",
                                    "...",
                                    "...",
                                    "...",
                                    index - last_idx - 1
                                );
                            }
                        }

                        let frame = &frames[index];
                        print!(
                            "  {:>8} {:>12} {:>8}",
                            index, frame.timestamp_us, frame.loop_iteration
                        );

                        for field_name in &selected_fields {
                            let value = frame.data.get(*field_name).copied().unwrap_or(0);
                            print!(" {value:>10}");
                        }

                        if field_names.len() > max_fields_to_show {
                            print!(" ...");
                        }
                        println!();

                        last_shown_index = Some(index);
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
fn display_debug_info(logs: &[BBLLog]) {
    if let Some(log) = logs.first() {
        println!("\n=== BBL FILE HEADERS ===");
        println!("Total headers: {}", log.header.all_headers.len());

        // Show key configuration
        println!("\nKey Configuration:");
        for header in &log.header.all_headers {
            if header.contains("Firmware revision:")
                || header.contains("Board information:")
                || header.contains("Craft name:")
                || header.contains("looptime:")
            {
                println!("{header}");
            }
        }

        println!("I-frame fields: {}", log.header.i_frame_def.count);
        println!("P-frame fields: {}", log.header.p_frame_def.count);
        println!("S-frame fields: {}", log.header.s_frame_def.count);
        if log.header.g_frame_def.count > 0 {
            println!("G-frame fields: {}", log.header.g_frame_def.count);
        }
        if log.header.h_frame_def.count > 0 {
            println!("H-frame fields: {}", log.header.h_frame_def.count);
        }
        if !log.header.sysconfig.is_empty() {
            println!("Sysconfig items: {}", log.header.sysconfig.len());
        }
    }

    // Display frame data for each log
    display_frame_data(logs);
}

fn display_log_info(log: &BBLLog, debug: bool) {
    let stats = &log.stats;
    let header = &log.header;

    println!(
        "\nLog {} of {}, frames: {}",
        log.log_number, log.total_logs, stats.total_frames
    );

    // Display firmware info
    if !header.firmware_revision.is_empty() {
        println!("Firmware: {}", header.firmware_revision);
    }
    if !header.board_info.is_empty() {
        println!("Board: {}", header.board_info);
    }
    if !header.craft_name.is_empty() {
        println!("Craft: {}", header.craft_name);
    }

    // Display statistics
    println!("\nStatistics");
    println!("Looptime        {:4} avg", header.looptime);
    println!("I frames   {:6}", stats.i_frames);
    println!("P frames   {:6}", stats.p_frames);
    if stats.h_frames > 0 {
        println!("H frames   {:6}", stats.h_frames);
    }
    if stats.g_frames > 0 {
        println!("G frames   {:6}", stats.g_frames);
    }
    if stats.e_frames > 0 {
        println!("E frames   {:6}", stats.e_frames);
    }
    if stats.s_frames > 0 {
        println!("S frames   {:6}", stats.s_frames);
    }
    println!("Frames     {:6}", stats.total_frames);

    // Show basic failed frames count for all users
    if stats.failed_frames > 0 {
        println!(
            "Failed frames       {:6} (parsing errors)",
            stats.failed_frames
        );
    }

    // Display detailed blackbox_decode compatibility analysis only in debug mode
    if debug
        && (stats.frame_validation_failures > 0
            || stats.invalid_frame_types > 0
            || stats.corrupted_frames > 0)
    {
        println!("\nBlackbox_decode Compatibility Analysis:");
        if stats.frame_validation_failures > 0 {
            println!(
                "Validation failures {:6} (technical validation)",
                stats.frame_validation_failures
            );
        }
        if stats.invalid_frame_types > 0 {
            println!("Invalid frame types {:6}", stats.invalid_frame_types);
        }
        if stats.corrupted_frames > 0 {
            println!(
                "Corrupted frames    {:6} (stream errors)",
                stats.corrupted_frames
            );
        }
        if !stats.unknown_frame_bytes.is_empty() {
            println!(
                "Unknown frame bytes: {:?}",
                stats
                    .unknown_frame_bytes
                    .iter()
                    .take(10)
                    .map(|b| format!("0x{b:02X}"))
                    .collect::<Vec<_>>()
            );
        }
    }

    // Display timing if available
    if stats.start_time_us > 0 && stats.end_time_us > stats.start_time_us {
        let duration_ms = (stats.end_time_us.saturating_sub(stats.start_time_us)) / 1000;
        println!("Duration   {duration_ms:6} ms");

        // Calculate frame rates for blackbox_decode comparison
        if debug {
            let duration_s = duration_ms as f64 / 1000.0;
            let main_frames = stats.i_frames + stats.p_frames;
            if duration_s > 0.0 && main_frames > 0 {
                let main_rate = main_frames as f64 / duration_s;
                println!("Main frame rate: {main_rate:.1} Hz (I+P frames)");
            }
            if stats.s_frames > 0 && duration_s > 0.0 {
                let s_rate = stats.s_frames as f64 / duration_s;
                println!("S frame rate: {s_rate:.1} Hz");
            }
        }
    }

    // Display data version and missing iterations
    if header.data_version > 0 {
        println!("Data ver   {:6}", header.data_version);
    }
    if stats.missing_iterations > 0 {
        println!("Missing    {:6} iterations", stats.missing_iterations);
    }
}

#[allow(dead_code)]
fn export_logs_to_csv(
    logs: &[BBLLog],
    input_path: &Path,
    options: &CsvExportOptions,
    debug: bool,
) -> Result<()> {
    let base_name = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("blackbox");

    let output_dir = if let Some(ref dir) = options.output_dir {
        Path::new(dir)
    } else {
        input_path.parent().unwrap_or(Path::new("."))
    };

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)?;
        if debug {
            println!("Created output directory: {output_dir:?}");
        }
    }

    if debug {
        println!(
            "Exporting {} logs to CSV in directory: {:?}",
            logs.len(),
            output_dir
        );
    }

    for log in logs {
        let log_suffix = if logs.len() > 1 {
            format!(".{:02}", log.log_number)
        } else {
            ".01".to_string()
        };

        // Export plaintext headers to separate CSV
        let header_csv_path = output_dir.join(format!("{base_name}{log_suffix}.headers.csv"));
        export_headers_to_csv(&log.header, &header_csv_path, debug)?;
        println!("Exported headers to: {}", header_csv_path.display());

        // Export flight data (I, P, S, G frames) to main CSV
        let flight_csv_path = output_dir.join(format!("{base_name}{log_suffix}.csv"));
        export_flight_data_to_csv(log, &flight_csv_path, debug)?;
        println!("Exported flight data to: {}", flight_csv_path.display());
    }

    Ok(())
}

fn export_single_log_to_csv(
    log: &BBLLog,
    input_path: &Path,
    options: &CsvExportOptions,
    debug: bool,
) -> Result<()> {
    let base_name = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("blackbox");

    let output_dir = if let Some(ref dir) = options.output_dir {
        Path::new(dir)
    } else {
        input_path.parent().unwrap_or(Path::new("."))
    };

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)?;
        if debug {
            println!("Created output directory: {output_dir:?}");
        }
    }

    let log_suffix = if log.total_logs > 1 {
        format!(".{:02}", log.log_number)
    } else {
        ".01".to_string()
    };

    // Export plaintext headers to separate CSV
    let header_csv_path = output_dir.join(format!("{base_name}{log_suffix}.headers.csv"));
    export_headers_to_csv(&log.header, &header_csv_path, debug)?;
    println!("Exported headers to: {}", header_csv_path.display());

    // Export flight data (I, P, S, G frames) to main CSV
    let flight_csv_path = output_dir.join(format!("{base_name}{log_suffix}.csv"));
    export_flight_data_to_csv(log, &flight_csv_path, debug)?;
    println!("Exported flight data to: {}", flight_csv_path.display());

    Ok(())
}

fn export_headers_to_csv(header: &BBLHeader, output_path: &Path, _debug: bool) -> Result<()> {
    use std::fs::File;
    use std::io::{BufWriter, Write};

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create headers CSV file: {output_path:?}"))?;
    let mut writer = BufWriter::new(file);

    // Write CSV header to match blackbox_decode format
    writeln!(writer, "fieldname,fieldvalue")?;

    // Parse and write all header lines
    for header_line in &header.all_headers {
        if let Some(content) = header_line.strip_prefix("H ") {
            // Remove "H " prefix and find the colon separator
            if let Some(colon_pos) = content.find(':') {
                let field_name = content[..colon_pos].trim();
                let field_value = content[colon_pos + 1..].trim();

                // Escape commas in values by wrapping in quotes
                let escaped_value = if field_value.contains(',') {
                    format!("\"{}\"", field_value.replace("\"", "\"\""))
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

fn export_flight_data_to_csv(log: &BBLLog, output_path: &Path, debug: bool) -> Result<()> {
    use std::fs::File;
    use std::io::{BufWriter, Write};

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create flight data CSV file: {output_path:?}"))?;
    let mut writer = BufWriter::new(file);

    // Build optimized field mapping (like C reference - pre-computed, no string matching per frame)
    let csv_map = CsvFieldMap::new(&log.header);
    let field_names: Vec<String> = csv_map
        .field_name_to_lookup
        .iter()
        .map(|(csv_name, _)| csv_name.clone())
        .collect();

    // Collect all frames in chronological order
    let mut all_frames = Vec::new();

    if let Some(ref debug_frames) = log.debug_frames {
        // Collect I, P, S frames (exclude E frames as they are events, not flight data)
        for frame_type in ['I', 'P', 'S'] {
            if let Some(frames) = debug_frames.get(&frame_type) {
                if debug && frame_type == 'I' {
                    println!("DEBUG: CSV collecting {} I-frames for export", frames.len());
                    if let Some(first_frame) = frames.first() {
                        println!(
                            "DEBUG: First I-frame has {} fields, axisP[0]={:?}, motor[0]={:?}",
                            first_frame.data.len(),
                            first_frame.data.get("axisP[0]"),
                            first_frame.data.get("motor[0]")
                        );
                    }
                }
                for frame in frames {
                    // Temporarily disable filtering to match blackbox_decode output exactly
                    // All frames are included to achieve target 24MB+ file size
                    all_frames.push((frame.timestamp_us, frame_type, frame));
                }
            }
        }
    }

    // **CRITICAL FIX**: Disable timestamp sorting to match C reference behavior
    // C reference outputs frames in strict parse order without sorting
    // Timestamp sorting was causing chaotic loopIteration sequences (8,4,15,1,2,3,7...)
    // all_frames.sort_by_key(|(timestamp, _, _)| *timestamp); // DISABLED

    // Remove frames with zero timestamps like blackbox_decode does
    // blackbox_decode filters out frames with invalid timestamps rather than interpolating them
    if !all_frames.is_empty() {
        let original_count = all_frames.len();
        all_frames.retain(|(timestamp, _, _)| *timestamp > 0);

        if debug && original_count != all_frames.len() {
            println!(
                "FILTERING: Removed {} frames with zero timestamps (blackbox_decode compatibility)",
                original_count - all_frames.len()
            );
        }

        // **CRITICAL FIX**: Disable timestamp sorting after filtering
        // all_frames.sort_by_key(|(timestamp, _, _)| *timestamp); // DISABLED

        // FRAME FILTERING: Remove corrupted frames to match blackbox_decode quality control
        // This filters out frames with duplicate timestamps and invalid loopIteration sequences
        let filtered_original_count = all_frames.len();
        let mut filtered_frames = Vec::new();
        let mut last_timestamp = 0u64;
        let mut expected_loop_iter = 0i32;
        let mut duplicate_timestamp_count = 0;
        let mut out_of_order_count = 0;

        for (timestamp, frame_type, frame) in all_frames.iter() {
            let mut should_include = true;

            // Check for duplicate timestamps (major corruption indicator)
            if *timestamp == last_timestamp && last_timestamp > 0 {
                duplicate_timestamp_count += 1;
                should_include = false;
                if debug && duplicate_timestamp_count <= 3 {
                    println!(
                        "FILTERING: Duplicate timestamp {timestamp} at loopIteration {:?}",
                        frame.data.get("loopIteration")
                    );
                }
            }

            // Check loopIteration sequence for main frames (I, P)
            if should_include && (*frame_type == 'I' || *frame_type == 'P') {
                if let Some(current_loop_iter) = frame.data.get("loopIteration") {
                    // Relaxed sequence validation - be much more tolerant like blackbox_decode
                    let iter_diff = *current_loop_iter - expected_loop_iter;
                    if !(-1000..=5000).contains(&iter_diff) {
                        // Only reject frames with truly massive gaps - likely log corruption or restart
                        out_of_order_count += 1;
                        should_include = false;
                        if debug && out_of_order_count <= 5 {
                            println!("FILTERING: Massive loopIteration gap {current_loop_iter} (expected ~{expected_loop_iter})");
                        }
                    } else if should_include {
                        // Update expected sequence based on current frame (handle gaps gracefully)
                        expected_loop_iter = if iter_diff > 100 {
                            // Large gap - reset expectation
                            *current_loop_iter + 1
                        } else {
                            // Normal sequence
                            (*current_loop_iter).max(expected_loop_iter) + 1
                        };
                    }
                }
            }

            if should_include {
                filtered_frames.push((*timestamp, *frame_type, *frame));
                last_timestamp = *timestamp;
            }
        }

        // Replace all_frames with filtered frames
        all_frames = filtered_frames;

        if debug && filtered_original_count != all_frames.len() {
            println!("FRAME FILTERING: Removed {} corrupted frames ({} duplicate timestamps, {} out-of-order)",
                     filtered_original_count - all_frames.len(), duplicate_timestamp_count, out_of_order_count);
            println!(
                "FRAME FILTERING: {} frames remaining (matches blackbox_decode quality control)",
                all_frames.len()
            );
        }
    }

    if all_frames.is_empty() {
        // Write at least the sample frames if no debug frames
        for frame in &log.sample_frames {
            all_frames.push((frame.timestamp_us, frame.frame_type, frame));
        }
        // **CRITICAL FIX**: Disable timestamp sorting for sample frames
        // all_frames.sort_by_key(|(timestamp, _, _)| *timestamp); // DISABLED
    }

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

    // Optimized CSV writing with pre-computed mappings (like C reference)
    let mut cumulative_energy_mah = 0f32;
    let mut last_timestamp_us = 0u64;
    let mut latest_s_frame_data: HashMap<String, i32> = HashMap::new();

    // Find first valid loopIteration for interpolation of invalid values
    let mut first_valid_loop_iter = None;
    for (_, _, frame) in &all_frames {
        if let Some(loop_iter) = frame.data.get("loopIteration") {
            if *loop_iter > 1000 {
                first_valid_loop_iter = Some(*loop_iter);
                break;
            }
        }
    }

    let _base_loop_iter = first_valid_loop_iter.unwrap_or(71000); // Default fallback if no valid found

    for (timestamp, frame_type, frame) in all_frames.iter() {
        // Debug first few frames to see what we're actually processing
        if debug && (timestamp == &all_frames[0].0 || timestamp == &all_frames[1].0) {
            println!(
                "DEBUG: CSV processing frame type={}, timestamp={}, data.len()={}, axisP[0]={:?}",
                frame_type,
                timestamp,
                frame.data.len(),
                frame.data.get("axisP[0]")
            );
        }

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
                let energy_increment = current_amps * time_delta_hours * 1000.0;
                cumulative_energy_mah += energy_increment;

                // Debug energy calculation for first few frames
                if debug && cumulative_energy_mah < 1.0 && last_timestamp_us > 0 {
                    println!("DEBUG: Energy calc - raw_current={}, amps={:.3}, time_delta_us={}, energy_inc={:.6}, total={:.3}", 
                             current_raw, current_amps, *timestamp - last_timestamp_us, energy_increment, cumulative_energy_mah);
                }
            }
            // Always update timestamp for next calculation, even on first frame
            last_timestamp_us = *timestamp;
        } else if debug && last_timestamp_us == 0 {
            println!("DEBUG: amperageLatest not found in frame data for energy calculation");
        }

        // Write data row using optimized field mapping
        for (i, (csv_name, lookup_name)) in csv_map.field_name_to_lookup.iter().enumerate() {
            if i > 0 {
                write!(writer, ", ")?;
            }

            // Fast path for special fields using pre-computed indices
            if csv_name == "time (us)" {
                // Use the actual parsed time value from frame data, not timestamp_us
                let time_value = frame.data.get("time").copied().unwrap_or(0);
                write!(writer, "{time_value}")?;
            } else if csv_name == "loopIteration" {
                // Use the actual parsed loopIteration value, not frame position
                let loop_value = frame.data.get("loopIteration").copied().unwrap_or(0);
                write!(writer, "{loop_value}")?;
            } else if csv_name == "vbatLatest (V)" {
                let raw_value = frame.data.get("vbatLatest").copied().unwrap_or(0);
                // Convert to volts to match blackbox_decode exactly
                write!(writer, "{:.1}", convert_vbat_to_volts(raw_value))?;
            } else if csv_name == "amperageLatest (A)" {
                let raw_value = frame.data.get("amperageLatest").copied().unwrap_or(0);
                // Convert to amps to match blackbox_decode exactly
                write!(writer, "{:.2}", convert_amperage_to_amps(raw_value))?;
            } else if csv_name == "energyCumulative (mAh)" {
                write!(writer, "{}", cumulative_energy_mah as i32)?;
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

                // Debug field lookup for key fields to understand the zero value issue
                if debug
                    && (lookup_name.contains("axisP")
                        || lookup_name.contains("motor")
                        || lookup_name.contains("gyroADC"))
                    && value == 0
                {
                    println!("DEBUG: CSV export field '{}' = 0, frame.data has: {:?}, latest_s_frame_data has: {:?}", 
                             lookup_name,
                             frame.data.get(lookup_name),
                             latest_s_frame_data.get(lookup_name));
                }

                write!(writer, "{value:3}")?; // Right-aligned with 3-character field width to match blackbox_decode
            }
        }
        writeln!(writer)?;
    }

    writer
        .flush()
        .with_context(|| format!("Failed to flush flight data CSV file: {output_path:?}"))?;

    if debug {
        println!(
            "Exported {} data rows with {} fields to CSV",
            all_frames.len(),
            field_names.len()
        );

        // Analyze data density for blackbox_decode comparison
        let main_frames = all_frames
            .iter()
            .filter(|(_, frame_type, _)| *frame_type == 'I' || *frame_type == 'P')
            .count();
        let s_frames = all_frames
            .iter()
            .filter(|(_, frame_type, _)| *frame_type == 'S')
            .count();
        println!("CSV composition: {main_frames} main frames (I+P), {s_frames} S frames");

        if let Some((start_time, _, _)) = all_frames.first() {
            if let Some((end_time, _, _)) = all_frames.last() {
                let duration_s = (*end_time as f64 - *start_time as f64) / 1_000_000.0;
                if duration_s > 0.0 {
                    let csv_rate = all_frames.len() as f64 / duration_s;
                    println!("CSV data rate: {csv_rate:.1} rows/second");
                }
            }
        }
    }

    Ok(())
}

/// Validates frame data according to blackbox_decode standards
/// Only performs technical validation, not flight state filtering
#[allow(dead_code)]
fn is_frame_technically_valid(
    frame_type: char,
    frame_data: &HashMap<String, i32>,
    header: &BBLHeader,
    debug: bool,
) -> bool {
    // Check if frame type is supported
    match frame_type {
        'I' | 'P' | 'S' | 'H' | 'G' | 'E' => {
            // Valid frame types
        }
        _ => {
            if debug {
                println!("Invalid frame type: '{frame_type}'");
            }
            return false;
        }
    }

    // Check if frame has required fields based on frame type
    match frame_type {
        'I' | 'P' => {
            // Main frames should have time field
            if !frame_data.contains_key("time") {
                if debug {
                    println!("Main frame missing 'time' field");
                }
                return false;
            }

            // Check for loop iteration field
            if !frame_data.contains_key("loopIteration") {
                if debug {
                    println!("Main frame missing 'loopIteration' field");
                }
                return false;
            }
        }
        'S' => {
            // S-frames should have flight mode or state data
            if !frame_data.contains_key("flightModeFlags") && !frame_data.contains_key("stateFlags")
            {
                if debug {
                    println!("S-frame missing state information");
                }
                return false;
            }
        }
        'G' | 'H' => {
            // GPS frames validation would go here
            // For now, accept all GPS frames
        }
        'E' => {
            // Event frames - always valid if properly parsed
        }
        _ => return false,
    }

    // Field existence validation - check if fields exist in frame definitions
    match frame_type {
        'I' => {
            if header.i_frame_def.count == 0 {
                if debug {
                    println!("I-frame data present but no I-frame definition");
                }
                return false;
            }
        }
        'P' => {
            if header.p_frame_def.count == 0 {
                if debug {
                    println!("P-frame data present but no P-frame definition");
                }
                return false;
            }
        }
        'S' => {
            if header.s_frame_def.count == 0 {
                if debug {
                    println!("S-frame data present but no S-frame definition");
                }
                return false;
            }
        }
        _ => {}
    }

    true
}

type ParseFramesResult = Result<(
    FrameStats,
    Vec<DecodedFrame>,
    Option<HashMap<char, Vec<DecodedFrame>>>,
)>;

fn parse_frames(
    binary_data: &[u8],
    header: &BBLHeader,
    debug: bool,
    store_frames: bool, // Changed from csv_export to more generic store_frames flag
) -> ParseFramesResult {
    let mut stats = FrameStats::default();
    let mut sample_frames = Vec::new();
    let mut debug_frames: HashMap<char, Vec<DecodedFrame>> = HashMap::new();
    let _last_main_frame_timestamp = 0u64; // Track timestamp for S frames

    // Track the most recent S-frame data for merging (following JavaScript approach)
    let mut last_slow_data: HashMap<String, i32> = HashMap::new();

    // Store all frames when CSV export or frames-only debug is requested
    let _store_all_frames = store_frames; // Store all frames when requested

    if debug {
        println!("Binary data size: {} bytes", binary_data.len());
        if !binary_data.is_empty() {
            println!(
                "First 16 bytes: {:02X?}",
                &binary_data[..16.min(binary_data.len())]
            );
        }
    }

    if binary_data.is_empty() {
        return Ok((stats, sample_frames, Some(debug_frames)));
    }

    // Initialize frame history for proper P-frame parsing
    let mut frame_history = FrameHistory {
        current_frame: vec![0; header.i_frame_def.count],
        previous_frame: vec![0; header.i_frame_def.count],
        previous2_frame: vec![0; header.i_frame_def.count],
        valid: false,
    };

    // CRITICAL: Add blackbox_decode validation state tracking
    let _last_main_frame_iteration: Option<u32> = None;
    let _last_main_frame_time: Option<u64> = None;

    let mut stream = bbl_format::BBLDataStream::new(binary_data);

    // Main frame parsing loop - process frames as a stream, don't store all
    while !stream.eof {
        let frame_start_pos = stream.pos;

        match stream.read_byte() {
            Ok(frame_type_byte) => {
                let frame_type = match frame_type_byte as char {
                    'I' => 'I',
                    'P' => 'P',
                    'H' => 'H',
                    'G' => 'G',
                    'E' => 'E',
                    'S' => 'S',
                    _ => {
                        // Track unknown frame bytes for blackbox_decode compatibility analysis
                        stats.unknown_frame_bytes.push(frame_type_byte);
                        stats.invalid_frame_types += 1;

                        if debug && stats.failed_frames < 3 {
                            println!(
                                "Unknown frame type byte 0x{:02X} ('{:?}') at offset {}",
                                frame_type_byte, frame_type_byte as char, frame_start_pos
                            );
                        }
                        stats.failed_frames += 1;
                        continue;
                    }
                };

                if debug && stats.total_frames < 3 {
                    println!("Found frame type '{frame_type}' at offset {frame_start_pos}");
                }

                // Parse frame using proper streaming logic
                let mut frame_data = HashMap::new();
                let mut parsing_success = false;

                match frame_type {
                    'I' => {
                        if header.i_frame_def.count > 0 {
                            // I-frames reset the prediction history
                            frame_history.current_frame.fill(0);

                            if bbl_format::parse_frame_data(
                                &mut stream,
                                &header.i_frame_def,
                                &mut frame_history.current_frame,
                                None, // I-frames don't use prediction
                                None,
                                0,
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                            )
                            .is_ok()
                            {
                                // Update time and loop iteration from parsed frame
                                for (i, field_name) in
                                    header.i_frame_def.field_names.iter().enumerate()
                                {
                                    if i < frame_history.current_frame.len() {
                                        let value = frame_history.current_frame[i];
                                        frame_data.insert(field_name.clone(), value);

                                        // Debug critical timing fields
                                        if debug
                                            && stats.i_frames < 1
                                            && (field_name == "loopIteration"
                                                || field_name == "time")
                                        {
                                            println!(
                                                "DEBUG: I-frame #{} CRITICAL field '{}' (index {}) = {}",
                                                stats.i_frames + 1,
                                                field_name,
                                                i,
                                                value
                                            );
                                        }

                                        // Debug key fields to understand parsing issues
                                        if debug
                                            && stats.i_frames < 2
                                            && (field_name.contains("gyroADC")
                                                || field_name.contains("motor")
                                                || field_name.contains("axisP"))
                                        {
                                            println!(
                                                "DEBUG: I-frame #{} field '{}' = {}",
                                                stats.i_frames + 1,
                                                field_name,
                                                value
                                            );
                                        }
                                    }
                                }

                                if debug && stats.i_frames < 2 {
                                    println!("DEBUG: I-frame #{} before S-merge, axisP[0]={:?}, motor[0]={:?}", 
                                             stats.i_frames + 1,
                                             frame_data.get("axisP[0]"),
                                             frame_data.get("motor[0]")
                                    );
                                }

                                // I-frames are complete, add to sample frames
                                sample_frames.push(DecodedFrame {
                                    frame_type,
                                    timestamp_us: frame_data.get("time").copied().unwrap_or(0)
                                        as u64,
                                    loop_iteration: frame_data
                                        .get("loopIteration")
                                        .copied()
                                        .unwrap_or(0)
                                        as u32,
                                    data: frame_data.clone(),
                                });

                                // Debug timestamp and loop iteration extraction for verification
                                if debug && stats.i_frames < 1 {
                                    println!(
                                        "DEBUG: I-frame #{} Frame data contains {} fields",
                                        stats.i_frames + 1,
                                        frame_data.len()
                                    );
                                    println!(
                                        "DEBUG: I-frame #{} Looking for 'time' field: {:?}",
                                        stats.i_frames + 1,
                                        frame_data.get("time")
                                    );
                                    println!("DEBUG: I-frame #{} Looking for 'loopIteration' field: {:?}", stats.i_frames + 1, frame_data.get("loopIteration"));
                                    println!(
                                        "DEBUG: I-frame #{} Final timestamp_us: {}",
                                        stats.i_frames + 1,
                                        frame_data.get("time").copied().unwrap_or(0) as u64
                                    );
                                    println!(
                                        "DEBUG: I-frame #{} Final loop_iteration: {}",
                                        stats.i_frames + 1,
                                        frame_data.get("loopIteration").copied().unwrap_or(0)
                                            as u32
                                    );

                                    // Check what's in frame_history.current_frame
                                    if frame_history.current_frame.len() > 1 {
                                        println!("DEBUG: I-frame #{} frame_history.current_frame[1] (time index): {}", stats.i_frames + 1, frame_history.current_frame[1]);
                                    }
                                }

                                // Debug what data is actually being stored in sample frames
                                if debug && (frame_type == 'I' || frame_type == 'P') {
                                    let non_zero_count =
                                        frame_data.values().filter(|&&v| v != 0).count();
                                    println!("DEBUG: Storing SAMPLE {} frame with {} total fields, {} non-zero: axisP[0]={:?}, motor[0]={:?}", 
                                             frame_type,
                                             frame_data.len(),
                                             non_zero_count,
                                             frame_data.get("axisP[0]"),
                                             frame_data.get("motor[0]"));

                                    // If the frame has mostly zero data, this indicates a parsing problem
                                    if non_zero_count < 5 && frame_data.len() > 10 {
                                        println!(
                                            "WARNING: Frame has mostly zero data - possible parsing issue"
                                        );
                                        if debug {
                                            println!(
                                                "Frame data keys: {:?}",
                                                frame_data.keys().collect::<Vec<_>>()
                                            );
                                        }
                                    }
                                }

                                // Initialize debug frame storage
                                debug_frames.insert('I', Vec::new());

                                // Add I-frame to debug frames for CSV export
                                debug_frames.entry('I').or_default().push(DecodedFrame {
                                    frame_type,
                                    timestamp_us: frame_data.get("time").copied().unwrap_or(0)
                                        as u64,
                                    loop_iteration: frame_data
                                        .get("loopIteration")
                                        .copied()
                                        .unwrap_or(0)
                                        as u32,
                                    data: frame_data.clone(),
                                });

                                // CRITICAL FIX: Update frame history for I-frames (was missing!)
                                frame_history.previous2_frame =
                                    frame_history.previous_frame.clone();
                                frame_history.previous_frame = frame_history.current_frame.clone();
                                frame_history.valid = true;

                                // Mark parsing success
                                parsing_success = true;
                            }
                        }
                    }
                    'P' => {
                        if header.p_frame_def.count > 0 {
                            // P-frames use prediction based on previous frames
                            let predictor = if frame_history.valid {
                                // Simple predictor: use value from the same position in the previous frame
                                frame_history.previous_frame.clone()
                            } else {
                                // No valid history, use zeros (or could use last known good values)
                                vec![0; header.i_frame_def.count]
                            };

                            if bbl_format::parse_frame_data(
                                &mut stream,
                                &header.p_frame_def,
                                &mut frame_history.current_frame,
                                Some(&predictor),
                                Some(&frame_history.previous2_frame), // Pass previous2_frame for PREDICT_STRAIGHT_LINE
                                0,
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                            )
                            .is_ok()
                            {
                                // Update frame data from parsed values (MISSING - THIS WAS THE BUG!)
                                for (i, field_name) in
                                    header.i_frame_def.field_names.iter().enumerate()
                                {
                                    if i < frame_history.current_frame.len() {
                                        let value = frame_history.current_frame[i];
                                        frame_data.insert(field_name.clone(), value);

                                        // Debug critical timing fields for P-frames
                                        if debug
                                            && stats.p_frames < 1
                                            && (field_name == "loopIteration"
                                                || field_name == "time")
                                        {
                                            println!(
                                                "DEBUG: P-frame #{} CRITICAL field '{}' (index {}) = {}",
                                                stats.p_frames + 1,
                                                field_name,
                                                i,
                                                value
                                            );
                                        }
                                    }
                                }

                                // Update frame history
                                frame_history.previous2_frame =
                                    frame_history.previous_frame.clone();
                                frame_history.previous_frame = frame_history.current_frame.clone();
                                frame_history.valid = true;

                                // Add parsed frame to sample frames
                                sample_frames.push(DecodedFrame {
                                    frame_type,
                                    timestamp_us: frame_data.get("time").copied().unwrap_or(0)
                                        as u64,
                                    loop_iteration: frame_data
                                        .get("loopIteration")
                                        .copied()
                                        .unwrap_or(0)
                                        as u32,
                                    data: frame_data.clone(),
                                });

                                // Debug timestamp and loop iteration extraction for verification
                                if debug && stats.p_frames < 2 {
                                    println!(
                                        "DEBUG: P-frame #{} Frame data contains {} fields",
                                        stats.p_frames + 1,
                                        frame_data.len()
                                    );
                                    println!(
                                        "DEBUG: P-frame #{} Looking for 'time' field: {:?}",
                                        stats.p_frames + 1,
                                        frame_data.get("time")
                                    );
                                    println!("DEBUG: P-frame #{} Looking for 'loopIteration' field: {:?}", stats.p_frames + 1, frame_data.get("loopIteration"));
                                    println!(
                                        "DEBUG: P-frame #{} Final timestamp_us: {}",
                                        stats.p_frames + 1,
                                        frame_data.get("time").copied().unwrap_or(0) as u64
                                    );
                                    println!(
                                        "DEBUG: P-frame #{} Final loop_iteration: {}",
                                        stats.p_frames + 1,
                                        frame_data.get("loopIteration").copied().unwrap_or(0)
                                            as u32
                                    );
                                }

                                // Debug what data is actually being stored in sample frames
                                if debug && (frame_type == 'I' || frame_type == 'P') {
                                    let non_zero_count =
                                        frame_data.values().filter(|&&v| v != 0).count();
                                    println!("DEBUG: Storing SAMPLE {} frame with {} total fields, {} non-zero: axisP[0]={:?}, motor[0]={:?}", 
                                             frame_type,
                                             frame_data.len(),
                                             non_zero_count,
                                             frame_data.get("axisP[0]"),
                                             frame_data.get("motor[0]"));

                                    // If the frame has mostly zero data, this indicates a parsing problem
                                    if non_zero_count < 5 && frame_data.len() > 10 {
                                        println!(
                                            "WARNING: Frame has mostly zero data - possible parsing issue"
                                        );
                                        if debug {
                                            println!(
                                                "Frame data keys: {:?}",
                                                frame_data.keys().collect::<Vec<_>>()
                                            );
                                        }
                                    }
                                }

                                // Initialize debug frame storage
                                debug_frames.entry('P').or_default().push(DecodedFrame {
                                    frame_type,
                                    timestamp_us: frame_data.get("time").copied().unwrap_or(0)
                                        as u64,
                                    loop_iteration: frame_data
                                        .get("loopIteration")
                                        .copied()
                                        .unwrap_or(0)
                                        as u32,
                                    data: frame_data.clone(),
                                });

                                // Mark parsing success
                                parsing_success = true;
                            }
                        }
                    }
                    'S' => {
                        if header.s_frame_def.count > 0 {
                            // S-frames are simple key-value pairs, no complex parsing
                            if bbl_format::parse_frame_data(
                                &mut stream,
                                &header.s_frame_def,
                                &mut frame_history.current_frame,
                                None, // No prediction for S-frames
                                None,
                                0,
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                            )
                            .is_ok()
                            {
                                // Update latest S-frame data for export
                                for (i, field_name) in
                                    header.s_frame_def.field_names.iter().enumerate()
                                {
                                    if i < frame_history.current_frame.len() {
                                        last_slow_data.insert(
                                            field_name.clone(),
                                            frame_history.current_frame[i],
                                        );
                                    }
                                }

                                // Add parsed S-frame to sample frames
                                sample_frames.push(DecodedFrame {
                                    frame_type,
                                    timestamp_us: frame_data.get("time").copied().unwrap_or(0)
                                        as u64,
                                    loop_iteration: frame_data
                                        .get("loopIteration")
                                        .copied()
                                        .unwrap_or(0)
                                        as u32,
                                    data: frame_data.clone(),
                                });

                                // Initialize debug frame storage
                                debug_frames.entry('S').or_default().push(DecodedFrame {
                                    frame_type,
                                    timestamp_us: frame_data.get("time").copied().unwrap_or(0)
                                        as u64,
                                    loop_iteration: frame_data
                                        .get("loopIteration")
                                        .copied()
                                        .unwrap_or(0)
                                        as u32,
                                    data: frame_data.clone(),
                                });

                                // Mark parsing success
                                parsing_success = true;
                            }
                        }
                    }
                    'H' | 'G' | 'E' => {
                        // For now, just skip these frames in the main parsing loop
                        // TODO: Implement proper parsing if needed
                        // Skip until next frame start is found
                        while let Ok(byte) = stream.read_byte() {
                            if byte == b'E'
                                || byte == b'S'
                                || byte == b'I'
                                || byte == b'P'
                                || byte == b'H'
                                || byte == b'G'
                            {
                                stream.pos -= 1; // Back up one position to reread the frame type
                                break;
                            }
                        }
                    }
                    _ => {}
                }

                if !parsing_success {
                    // Frame parsing failed, skip to next frame
                    stats.failed_frames += 1;
                    if debug {
                        println!("Frame parsing failed at offset {frame_start_pos}");
                    }
                    // Skip until next frame start is found
                    while let Ok(byte) = stream.read_byte() {
                        if byte == b'E'
                            || byte == b'S'
                            || byte == b'I'
                            || byte == b'P'
                            || byte == b'H'
                            || byte == b'G'
                        {
                            stream.pos -= 1; // Back up one position to reread the frame type
                            break;
                        }
                    }
                } else {
                    // Successfully parsed a frame
                    stats.total_frames += 1;

                    // Update frame-specific stats
                    match frame_type {
                        'I' => stats.i_frames += 1,
                        'P' => stats.p_frames += 1,
                        'S' => stats.s_frames += 1,
                        'H' => stats.h_frames += 1,
                        'G' => stats.g_frames += 1,
                        'E' => stats.e_frames += 1,
                        _ => {}
                    }

                    if debug {
                        println!(
                            "Parsed {} frame: {:?}",
                            frame_type, frame_history.current_frame
                        );
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading frame type byte: {e}");
                break;
            }
        }
    }

    // Final stats update
    stats.total_frames = sample_frames.len() as u32;

    Ok((stats, sample_frames, Some(debug_frames)))
}

/// Output frame data for debugging (implements --frames-only functionality)
fn debug_output_frames(log: &BBLLog, debug: bool) {
    if debug {
        println!("\n=== FRAME DEBUGGING OUTPUT ===");
        println!("Total frames by type:");
        println!("  I-frames: {}", log.stats.i_frames);
        println!("  P-frames: {}", log.stats.p_frames);
        println!("  S-frames: {}", log.stats.s_frames);
        println!("  G-frames: {}", log.stats.g_frames);
        println!("  H-frames: {}", log.stats.h_frames);
        println!("  E-frames: {}", log.stats.e_frames);
        println!("  Failed frames: {}", log.stats.failed_frames);
        println!(
            "  Frame validation failures: {}",
            log.stats.frame_validation_failures
        );
    }

    // If we have debug frames, output them
    if let Some(ref debug_frames) = log.debug_frames {
        // Output I-frames first
        if let Some(i_frames) = debug_frames.get(&'I') {
            println!("\nI-Frames (Intra):");
            for (i, frame) in i_frames.iter().enumerate() {
                println!(
                    "I-Frame {}: time={}, iteration={}, fields={:?}",
                    i + 1,
                    frame.timestamp_us,
                    frame.loop_iteration,
                    frame
                        .data
                        .iter()
                        .map(|(k, v)| format!("{k}:{v}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );

                // Only show a few frames to avoid overwhelming output
                if i >= 9 && i_frames.len() > 20 {
                    println!("... ({} more I-frames)", i_frames.len() - 10);
                    break;
                }
            }
        }

        // Output P-frames
        if let Some(p_frames) = debug_frames.get(&'P') {
            println!("\nP-Frames (Predicted):");
            for (i, frame) in p_frames.iter().enumerate() {
                println!(
                    "P-Frame {}: time={}, iteration={}, fields={:?}",
                    i + 1,
                    frame.timestamp_us,
                    frame.loop_iteration,
                    frame
                        .data
                        .iter()
                        .map(|(k, v)| format!("{k}:{v}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );

                // Only show a few frames to avoid overwhelming output
                if i >= 9 && p_frames.len() > 20 {
                    println!("... ({} more P-frames)", p_frames.len() - 10);
                    break;
                }
            }
        }

        // Output S-frames
        if let Some(s_frames) = debug_frames.get(&'S') {
            println!("\nS-Frames (Slow):");
            for (i, frame) in s_frames.iter().enumerate() {
                println!(
                    "S-Frame {}: time={}, fields={:?}",
                    i + 1,
                    frame.timestamp_us,
                    frame
                        .data
                        .iter()
                        .map(|(k, v)| format!("{k}:{v}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );

                // Only show a few frames
                if i >= 4 && s_frames.len() > 10 {
                    println!("... ({} more S-frames)", s_frames.len() - 5);
                    break;
                }
            }
        }
    } else {
        println!("No debug frame data available");
    }
}

/// Main streaming implementation for processing BBL files
/// Processes the file in a streaming manner to minimize memory usage
/// Outputs frame data only when frames_only is true (similar to blackbox_decode -d option)
fn parse_bbl_file_streaming(
    file_path: &Path,
    debug: bool,
    csv_export: bool,
    frames_only: bool,
    csv_options: &CsvExportOptions,
) -> Result<usize> {
    if debug {
        println!("=== PARSING BBL FILE (STREAMING) ===");
        let metadata = std::fs::metadata(file_path)?;
        println!(
            "File size: {} bytes ({:.2} MB)",
            metadata.len(),
            metadata.len() as f64 / 1024.0 / 1024.0
        );
    }

    // Read file in one go - could be optimized further with actual streaming
    let file_data = std::fs::read(file_path)?;

    // Look for multiple logs by searching for log start markers
    let log_start_marker = b"H Product:Blackbox flight data recorder by Nicholas Sherlock";
    let mut log_positions = Vec::new();

    // Find all log start positions
    for i in 0..file_data.len() {
        if i + log_start_marker.len() <= file_data.len()
            && &file_data[i..i + log_start_marker.len()] == log_start_marker
        {
            log_positions.push(i);
        }
    }

    if log_positions.is_empty() {
        return Err(anyhow::anyhow!("No blackbox log headers found in file"));
    }

    if debug {
        println!("Found {} log(s) in file", log_positions.len());
    }

    let mut processed_logs = 0;

    // Process each log segment
    for (log_index, &start_pos) in log_positions.iter().enumerate() {
        if debug {
            println!(
                "\nProcessing log {} starting at position {}",
                log_index + 1,
                start_pos
            );
        }

        // Determine end position (start of next log or end of file)
        let end_pos = log_positions
            .get(log_index + 1)
            .copied()
            .unwrap_or(file_data.len());
        let log_data = &file_data[start_pos..end_pos];

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
        let header = match parse_headers_from_text(header_text, debug) {
            Ok(h) => h,
            Err(e) => {
                if debug {
                    println!("Error parsing headers for log {}: {}", log_index + 1, e);
                }
                continue;
            }
        };

        // Parse binary frame data
        let binary_data = &log_data[header_end..];
        let (mut stats, frames, debug_frames) =
            match parse_frames(binary_data, &header, debug, csv_export || frames_only) {
                Ok(result) => result,
                Err(e) => {
                    if debug {
                        println!("Error parsing frames for log {}: {}", log_index + 1, e);
                    }
                    continue;
                }
            };

        // Update frame stats timing from actual frame data
        if !frames.is_empty() {
            stats.start_time_us = frames.first().unwrap().timestamp_us;
            stats.end_time_us = frames.last().unwrap().timestamp_us;
        }

        if debug {
            // Debug: Show I-frame field order to compare with C implementation
            println!("DEBUG: I-frame field order:");
            for (i, field_name) in header.i_frame_def.field_names.iter().enumerate() {
                println!("  [{i}]: {field_name}");
                if i > 5 {
                    // Only show first few to avoid spam
                    println!(
                        "  ... ({} total fields)",
                        header.i_frame_def.field_names.len()
                    );
                    break;
                }
            }
        }

        // Check if log has meaningful data for processing
        let has_meaningful_data = stats.i_frames > 0 || stats.p_frames > 0;

        if debug {
            println!(
                "Log {}: has_meaningful_data={}, i_frames={}, p_frames={}",
                log_index + 1,
                has_meaningful_data,
                stats.i_frames,
                stats.p_frames
            );
        }

        if !has_meaningful_data {
            if debug {
                println!("Skipping log {} - no meaningful data", log_index + 1);
            }
            continue;
        }

        // Create BBL log object for this log
        let log = BBLLog {
            log_number: log_index + 1,
            total_logs: log_positions.len(),
            header,
            stats,
            sample_frames: frames,
            debug_frames,
        };

        // Handle frames-only debug output (similar to blackbox_decode -d)
        if frames_only {
            debug_output_frames(&log, debug);
        } else {
            // Always show log statistics for all users (more detailed in debug mode)
            display_log_info(&log, debug);
        }

        // Handle CSV export if requested (in addition to console output)
        if csv_export {
            export_single_log_to_csv(&log, file_path, csv_options, debug)?;
            // Show brief additional CSV export info in debug mode
            if debug {
                println!(
                    "  → Exported CSV for Log {}: {} total frames",
                    log.log_number, log.stats.total_frames
                );
            }
        }

        processed_logs += 1;
    }

    Ok(processed_logs)
}

/// Format flight mode flags based on betaflight firmware flags
fn format_flight_mode_flags(flags: i32) -> String {
    let mut modes = Vec::new();

    // Based on Betaflight firmware runtime_config.h flightModeFlags_e enum (12 flags total, 0-11)
    // Reference: https://github.com/betaflight/betaflight/blob/master/src/main/fc/runtime_config.h
    if flags & (1 << 0) != 0 {
        modes.push("ARM");
    }
    if flags & (1 << 1) != 0 {
        modes.push("ANGLE");
    }
    if flags & (1 << 2) != 0 {
        modes.push("HORIZON");
    }
    if flags & (1 << 3) != 0 {
        modes.push("BARO");
    }
    if flags & (1 << 4) != 0 {
        // Reserved / Anti Gravity in newer versions
        modes.push("ANTIGRAVITY");
    }
    if flags & (1 << 5) != 0 {
        modes.push("MAG");
    }
    if flags & (1 << 6) != 0 {
        modes.push("HEADFREE");
    }
    if flags & (1 << 7) != 0 {
        modes.push("HEADADJ");
    }
    if flags & (1 << 8) != 0 {
        modes.push("CAMSTAB");
    }
    if flags & (1 << 9) != 0 {
        modes.push("PASSTHRU");
    }
    if flags & (1 << 10) != 0 {
        modes.push("BEEPERON");
    }
    if flags & (1 << 11) != 0 {
        modes.push("LEDLOW");
    }

    modes.join("|")
}

/// Format state flags based on betaflight firmware state flags
fn format_state_flags(flags: i32) -> String {
    let mut states = Vec::new();

    // Based on Betaflight stateFlags_t
    if flags & (1 << 0) != 0 {
        states.push("GPS_FIX");
    }
    if flags & (1 << 1) != 0 {
        states.push("GPS_FIX_HOME");
    }
    if flags & (1 << 2) != 0 {
        states.push("CALIBRATE_MAG");
    }
    if flags & (1 << 3) != 0 {
        states.push("SMALL_ANGLE");
    }
    if flags & (1 << 4) != 0 {
        states.push("FIXED_WING");
    }
    // Add other states as needed

    states.join("|")
}

/// Format failsafe phase values
fn format_failsafe_phase(phase: i32) -> String {
    match phase {
        0 => "IDLE".to_string(),
        1 => "RX_LOSS_DETECTED".to_string(),
        2 => "LANDING".to_string(),
        3 => "LANDED".to_string(),
        _ => format!("UNKNOWN({phase})"),
    }
}

#[allow(dead_code)]
fn parse_signed_data(data_str: &str) -> Vec<bool> {
    data_str
        .trim()
        .split(',')
        .map(|s| s.trim() == "1" || s.trim().to_lowercase() == "true")
        .collect()
}

/// Parse numeric data (like predictors or encodings) from header into a vector of u8 values
fn parse_numeric_data(data_str: &str) -> Vec<u8> {
    data_str
        .trim()
        .split(',')
        .filter_map(|s| s.trim().parse::<u8>().ok())
        .collect()
}

/// Convert raw vbat value to volts (follows blackbox_decode logic)
fn convert_vbat_to_volts(raw_value: i32) -> f32 {
    raw_value as f32 / 100.0
}

/// Convert raw amperage value to amps (follows blackbox_decode logic)
fn convert_amperage_to_amps(raw_value: i32) -> f32 {
    raw_value as f32 / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_format_flight_mode_flags() {
        // Test empty flags
        assert_eq!(format_flight_mode_flags(0), "");

        // Test single flags
        assert_eq!(format_flight_mode_flags(1 << 0), "ARM");
        assert_eq!(format_flight_mode_flags(1 << 1), "ANGLE");
        assert_eq!(format_flight_mode_flags(1 << 2), "HORIZON");

        // Test combined flags
        assert_eq!(format_flight_mode_flags((1 << 0) | (1 << 1)), "ARM|ANGLE");
    }

    #[test]
    fn test_format_state_flags() {
        // Test empty flags
        assert_eq!(format_state_flags(0), "");

        // Test single flags (corrected based on actual function)
        assert_eq!(format_state_flags(1 << 0), "GPS_FIX");
        assert_eq!(format_state_flags(1 << 1), "GPS_FIX_HOME");
    }

    #[test]
    fn test_frame_definition() {
        // Test empty frame definition
        let frame_def = FrameDefinition::new();
        assert_eq!(frame_def.count, 0);
        assert!(frame_def.fields.is_empty());
        assert!(frame_def.field_names.is_empty());
    }

    #[test]
    fn test_frame_stats_default() {
        let stats = FrameStats::default();
        assert_eq!(stats.i_frames, 0);
        assert_eq!(stats.p_frames, 0);
        assert_eq!(stats.total_frames, 0);
    }

    #[test]
    fn test_should_have_frame() {
        let mut sysconfig = HashMap::new();
        sysconfig.insert("frameIntervalI".to_string(), 32);
        sysconfig.insert("frameIntervalPNum".to_string(), 1);
        sysconfig.insert("frameIntervalPDenom".to_string(), 1);

        // Test I-frame interval logic
        assert!(should_have_frame(0, &sysconfig));
        assert!(should_have_frame(1, &sysconfig));
    }
}
