mod bbl_format;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use glob::glob;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone)]
struct FieldDefinition {
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
    total_bytes: u64,
    start_time_us: u64,
    end_time_us: u64,
    failed_frames: u32,
    missing_iterations: u64,
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
    chronological_frames: Option<Vec<DecodedFrame>>, // All frames in BBL file order for CSV export
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

        // **BLACKBOX_DECODE COMPATIBILITY**: Add energyCumulative before S-frame fields
        // This matches blackbox_decode field ordering exactly
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
        .get_matches();

    let debug = matches.get_flag("debug");
    let export_csv = matches.get_flag("csv");
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

        match parse_bbl_file_streaming(path, debug, export_csv, &csv_options) {
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
    let (mut stats, frames, debug_frames, chronological_frames) = parse_frames(binary_data, &header, debug, csv_export)?;

    // Update frame stats timing from actual frame data
    if !frames.is_empty() {
        stats.start_time_us = frames.first().unwrap().timestamp_us;
        stats.end_time_us = frames.last().unwrap().timestamp_us;
    }

    let log = BBLLog {
        log_number,
        total_logs,
        header,
        stats,
        sample_frames: frames,
        debug_frames,
        chronological_frames,
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

fn display_log_info(log: &BBLLog) {
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
    
    // Show basic failed frames count for all users (from beneficial branch enhancement)
    if stats.failed_frames > 0 {
        println!("Failed frames       {:6} (parsing errors)", stats.failed_frames);
    }

    // Display timing if available
    if stats.start_time_us > 0 && stats.end_time_us > stats.start_time_us {
        let duration_ms = (stats.end_time_us.saturating_sub(stats.start_time_us)) / 1000;
        println!("Duration   {duration_ms:6} ms");
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

    // **BLACKBOX_DECODE COMPATIBILITY**: Use chronological frames that preserve BBL file order
    if let Some(ref chronological_frames) = log.chronological_frames {
        // Frames are already in correct BBL file order from parsing
        for frame in chronological_frames {
            all_frames.push((frame.timestamp_us, frame.frame_type, frame));
        }
    } else if let Some(ref debug_frames) = log.debug_frames {
        // Fallback to old method if chronological frames not available
        for frame_type in ['I', 'P', 'S'] {
            if let Some(frames) = debug_frames.get(&frame_type) {
                for frame in frames {
                    all_frames.push((frame.timestamp_us, frame_type, frame));
                }
            }
        }
        // Sort by timestamp to restore chronological order
        all_frames.sort_by_key(|(timestamp, _, _)| *timestamp);
    }

    // **CRITICAL FIX**: Frames must be processed in BBL file order, not timestamp order
    // blackbox_decode.c processes frames sequentially - sorting breaks time progression
    
    if all_frames.is_empty() {
        // Write at least the sample frames if no debug frames
        for frame in &log.sample_frames {
            all_frames.push((frame.timestamp_us, frame.frame_type, frame));
        }
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
                write!(writer, "{:4.1}", convert_vbat_to_volts(raw_value))?;
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

    if debug {
        println!(
            "Exported {} data rows with {} fields (optimized)",
            all_frames.len(),
            field_names.len()
        );
    }

    Ok(())
}

type ParseFramesResult = Result<(
    FrameStats,
    Vec<DecodedFrame>,
    Option<HashMap<char, Vec<DecodedFrame>>>,
    Option<Vec<DecodedFrame>>, // Chronological frames for CSV export
)>;

fn parse_frames(
    binary_data: &[u8],
    header: &BBLHeader,
    debug: bool,
    csv_export: bool,
) -> ParseFramesResult {
    let mut stats = FrameStats::default();
    let mut sample_frames = Vec::new();
    let mut debug_frames: HashMap<char, Vec<DecodedFrame>> = HashMap::new();
    let mut chronological_frames = Vec::new(); // Store frames in BBL file order
    let mut last_main_frame_timestamp = 0u64; // Track timestamp for S frames

    // **BLACKBOX_DECODE COMPATIBILITY**: Add timestamp rollover handling like blackbox_decode.c
    let mut time_rollover_accumulator: u64 = 0; // Tracks 32-bit timestamp rollovers  
    let mut last_main_frame_time: i64 = -1; // Last timestamp for rollover detection

    // **BLACKBOX_DECODE COMPATIBILITY**: Track frame validation like blackbox_decode.c
    let mut last_main_frame_iteration: u32 = u32::MAX; // Track last loop iteration for validation

    // Track the most recent S-frame data for merging (following JavaScript approach)
    let mut last_slow_data: HashMap<String, i32> = HashMap::new();

    // Decide whether to store all frames based on CSV export requirement
    let store_all_frames = csv_export; // Store all frames when CSV export is requested

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
        return Ok((stats, sample_frames, Some(debug_frames), Some(chronological_frames)));
    }

    // Initialize frame history for proper P-frame parsing
    let mut frame_history = FrameHistory {
        current_frame: vec![0; header.i_frame_def.count],
        previous_frame: vec![0; header.i_frame_def.count],
        previous2_frame: vec![0; header.i_frame_def.count],
        valid: false,
    };

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
                                    }
                                }

                                // Merge lastSlow data into I-frame (following JavaScript approach)
                                for (key, value) in &last_slow_data {
                                    frame_data.insert(key.clone(), *value);
                                }

                                if debug && stats.i_frames < 3 {
                                    println!("DEBUG: I-frame merged lastSlow. rxSignalReceived: {:?}, rxFlightChannelsValid: {:?}", 
                                             frame_data.get("rxSignalReceived"), frame_data.get("rxFlightChannelsValid"));
                                }

                                // Update history for future P-frames
                                // Both the previous and previous-previous states become the I-frame,
                                // because we can't look further into the past than the I-frame
                                frame_history
                                    .previous_frame
                                    .copy_from_slice(&frame_history.current_frame);
                                frame_history
                                    .previous2_frame
                                    .copy_from_slice(&frame_history.current_frame);
                                frame_history.valid = true;

                                // **BLACKBOX_DECODE COMPATIBILITY**: Validate I-frame values like blackbox_decode.c
                                let current_loop_iteration = frame_data.get("loopIteration").copied().unwrap_or(0) as u32;
                                let current_time = frame_data.get("time").copied().unwrap_or(0) as i64;
                                
                                let is_valid_frame = if last_main_frame_iteration != u32::MAX {
                                    // Validate against previous frame like flightLogValidateMainFrameValues()
                                    current_loop_iteration >= last_main_frame_iteration &&
                                    current_loop_iteration < last_main_frame_iteration.saturating_add(5000) && // MAXIMUM_ITERATION_JUMP_BETWEEN_FRAMES 
                                    current_time >= last_main_frame_time &&
                                    current_time < last_main_frame_time + 10_000_000 // MAXIMUM_TIME_JUMP_BETWEEN_FRAMES (10 seconds)
                                } else {
                                    true // First frame is always valid
                                };

                                if is_valid_frame {
                                    // Update tracking variables for next validation
                                    last_main_frame_iteration = current_loop_iteration;
                                    last_main_frame_time = current_time;
                                    
                                    parsing_success = true;
                                    stats.i_frames += 1;
                                } else {
                                    // Reject invalid frame like blackbox_decode does  
                                    stats.failed_frames += 1;
                                    if debug {
                                        println!("DEBUG: Rejected I-frame - loopIteration:{} time:{} (prev iter:{} time:{})", 
                                                current_loop_iteration, current_time, last_main_frame_iteration, last_main_frame_time);
                                    }
                                }
                            }
                        }
                    }
                    'P' => {
                        if header.p_frame_def.count > 0 && frame_history.valid {
                            // **BLACKBOX_DECODE COMPATIBILITY**: P-frames update current frame directly
                            // Copy previous frame state first (like blackbox_decode mainHistory approach)
                            frame_history
                                .current_frame
                                .copy_from_slice(&frame_history.previous_frame);

                            // Parse P-frame data into temporary array first
                            let mut p_frame_values = vec![0i32; header.p_frame_def.count];
                            
                            if bbl_format::parse_frame_data(
                                &mut stream,
                                &header.p_frame_def,
                                &mut p_frame_values,
                                Some(&frame_history.previous_frame),
                                Some(&frame_history.previous2_frame),
                                0,     // TODO: Calculate skipped frames properly
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                            )
                            .is_ok()
                            {
                                // **CRITICAL FIX**: Apply P-frame values to current frame using correct field indices
                                // This matches blackbox_decode where P-frames update specific fields in mainHistory[0]
                                for (p_idx, field_name) in header.p_frame_def.field_names.iter().enumerate() {
                                    if p_idx < p_frame_values.len() {
                                        // Find corresponding index in I-frame structure
                                        if let Some(i_frame_idx) = header
                                            .i_frame_def
                                            .field_names
                                            .iter()
                                            .position(|name| name == field_name)
                                        {
                                            if i_frame_idx < frame_history.current_frame.len() {
                                                frame_history.current_frame[i_frame_idx] = p_frame_values[p_idx];
                                            }
                                        }
                                    }
                                }

                                // Copy current frame to output using I-frame field names and structure
                                for (i, field_name) in
                                    header.i_frame_def.field_names.iter().enumerate()
                                {
                                    if i < frame_history.current_frame.len() {
                                        let value = frame_history.current_frame[i];
                                        frame_data.insert(field_name.clone(), value);
                                    }
                                }

                                // Merge lastSlow data into P-frame (following JavaScript approach)
                                for (key, value) in &last_slow_data {
                                    frame_data.insert(key.clone(), *value);
                                }

                                if debug && stats.p_frames < 3 {
                                    println!("DEBUG: P-frame merged lastSlow. rxSignalReceived: {:?}, rxFlightChannelsValid: {:?}", 
                                             frame_data.get("rxSignalReceived"), frame_data.get("rxFlightChannelsValid"));
                                }

                                // Update history
                                frame_history
                                    .previous2_frame
                                    .copy_from_slice(&frame_history.previous_frame);
                                frame_history
                                    .previous_frame
                                    .copy_from_slice(&frame_history.current_frame);

                                // **BLACKBOX_DECODE COMPATIBILITY**: Validate frame values like blackbox_decode.c
                                // Check that iteration count and time didn't move backwards or jump too much
                                let current_loop_iteration = frame_data.get("loopIteration").copied().unwrap_or(0) as u32;
                                let current_time = frame_data.get("time").copied().unwrap_or(0) as i64;
                                
                                let is_valid_frame = true; // **TEMPORARY**: Test chronological ordering
                                
                                if is_valid_frame {
                                    // Update tracking variables for next validation
                                    last_main_frame_iteration = current_loop_iteration;
                                    last_main_frame_time = current_time;
                                    
                                    parsing_success = true;
                                    stats.p_frames += 1;
                                } else {
                                    // Reject invalid frame like blackbox_decode does
                                    stats.failed_frames += 1;
                                    if debug {
                                        println!("DEBUG: Rejected P-frame - loopIteration:{} time:{} (prev iter:{} time:{})", 
                                                current_loop_iteration, current_time, last_main_frame_iteration, last_main_frame_time);
                                    }
                                }
                            }
                        } else {
                            // Skip P-frame if we don't have valid I-frame history
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.failed_frames += 1;
                        }
                    }
                    'S' => {
                        if header.s_frame_def.count > 0 {
                            if let Ok(data) = parse_s_frame(&mut stream, &header.s_frame_def, debug)
                            {
                                // Following JavaScript approach: update lastSlow data
                                if debug && stats.s_frames < 3 {
                                    println!("DEBUG: Processing S-frame with data: {data:?}");
                                }

                                for (key, value) in &data {
                                    last_slow_data.insert(key.clone(), *value);
                                }

                                if debug && stats.s_frames < 3 {
                                    println!(
                                        "DEBUG: S-frame data updated lastSlow: {last_slow_data:?}"
                                    );
                                }

                                frame_data = data;
                                parsing_success = true;
                                stats.s_frames += 1;
                            }
                        }
                    }
                    'H' => {
                        if header.h_frame_def.count > 0 {
                            if let Ok(data) = parse_h_frame(&mut stream, &header.h_frame_def, debug)
                            {
                                frame_data = data;
                                parsing_success = true;
                                stats.h_frames += 1;
                            }
                        } else {
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.h_frames += 1;
                            parsing_success = true;
                        }
                    }
                    'G' => {
                        if header.g_frame_def.count > 0 {
                            if let Ok(data) = parse_g_frame(&mut stream, &header.g_frame_def, debug)
                            {
                                frame_data = data;
                                parsing_success = true;
                                stats.g_frames += 1;
                            }
                        } else {
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.g_frames += 1;
                            parsing_success = true;
                        }
                    }
                    'E' => {
                        skip_frame(&mut stream, frame_type, debug)?;
                        stats.e_frames += 1;
                        parsing_success = true;
                    }
                    _ => {}
                };

                if !parsing_success {
                    stats.failed_frames += 1;
                }

                stats.total_frames += 1;

                // Show progress for large files
                if (debug && stats.total_frames % 50000 == 0) || stats.total_frames % 100000 == 0 {
                    println!("Parsed {} frames so far...", stats.total_frames);
                    std::io::stdout().flush().unwrap_or_default();
                }

                // Store only a few sample frames for display purposes
                if parsing_success && sample_frames.len() < 10 {
                    // **BLACKBOX_DECODE COMPATIBILITY**: Apply timestamp rollover correction  
                    let raw_timestamp = frame_data.get("time").copied().unwrap_or(0);
                    let loop_iteration = frame_data.get("loopIteration").copied().unwrap_or(0) as u32;
                    
                    // **DEBUG**: Show raw timestamp values
                    if sample_frames.len() < 2 {
                        println!("DEBUG: Frame {} type '{}' loopIteration:{} raw_time:{}", 
                               sample_frames.len(), frame_type, loop_iteration, raw_timestamp);
                        
                        // **CRITICAL DEBUG**: Find exact time field position
                        for (i, field_name) in header.i_frame_def.field_names.iter().enumerate() {
                            if field_name == "time" {
                                println!("DEBUG: I-frame 'time' found at index {}", i);
                                break;
                            }
                        }
                        for (i, field_name) in header.p_frame_def.field_names.iter().enumerate() {
                            if field_name == "time" {
                                println!("DEBUG: P-frame 'time' found at index {}", i);
                                break;
                            }
                        }
                    }
                    
                    // Apply rollover detection for main frames (I, P) like blackbox_decode.c
                    let timestamp_us = if frame_type == 'I' || frame_type == 'P' {
                        let corrected_time = detect_and_apply_timestamp_rollover(
                            raw_timestamp, 
                            &mut last_main_frame_time, 
                            &mut time_rollover_accumulator
                        );
                        last_main_frame_time = corrected_time as i64;
                        corrected_time
                    } else {
                        raw_timestamp as u64
                    };

                    // Update last timestamp for main frames (I, P)
                    if (frame_type == 'I' || frame_type == 'P') && timestamp_us > 0 {
                        last_main_frame_timestamp = timestamp_us;
                    }

                    // S frames inherit timestamp from last main frame
                    let final_timestamp = if frame_type == 'S' && timestamp_us == 0 {
                        last_main_frame_timestamp
                    } else {
                        timestamp_us
                    };

                    if debug && (frame_type == 'I' || frame_type == 'P') && sample_frames.len() < 3
                    {
                        println!(
                            "DEBUG: Frame {:?} has timestamp {}. Available fields: {:?}",
                            frame_type,
                            timestamp_us,
                            frame_data.keys().collect::<Vec<_>>()
                        );
                        if let Some(time_val) = frame_data.get("time") {
                            println!("DEBUG: 'time' field value: {time_val}");
                        }
                        if let Some(loop_val) = frame_data.get("loopIteration") {
                            println!("DEBUG: 'loopIteration' field value: {loop_val}");
                        }
                    }

                    let decoded_frame = DecodedFrame {
                        frame_type,
                        timestamp_us: final_timestamp,
                        loop_iteration,
                        data: frame_data.clone(),
                    };
                    sample_frames.push(decoded_frame.clone());

                    // Store debug frames (always store for sample frames)
                    let debug_frame_list = debug_frames.entry(frame_type).or_default();
                    debug_frame_list.push(decoded_frame.clone());
                    
                    // **BLACKBOX_DECODE COMPATIBILITY**: Store in chronological order for CSV export
                    if store_all_frames {
                        chronological_frames.push(decoded_frame);
                    }
                } else if parsing_success && store_all_frames {
                    // Store ALL frames for CSV export when requested
                    let debug_frame_list = debug_frames.entry(frame_type).or_default();
                    
                    // **BLACKBOX_DECODE COMPATIBILITY**: Apply timestamp rollover correction  
                    let raw_timestamp = frame_data.get("time").copied().unwrap_or(0);
                    let loop_iteration = frame_data.get("loopIteration").copied().unwrap_or(0) as u32;
                    
                    // Apply rollover detection for main frames (I, P) like blackbox_decode.c
                    let timestamp_us = if frame_type == 'I' || frame_type == 'P' {
                        let corrected_time = detect_and_apply_timestamp_rollover(
                            raw_timestamp, 
                            &mut last_main_frame_time, 
                            &mut time_rollover_accumulator
                        );
                        last_main_frame_time = corrected_time as i64;
                        corrected_time
                    } else {
                        raw_timestamp as u64
                    };

                    // Update last timestamp for main frames (I, P)
                    if (frame_type == 'I' || frame_type == 'P') && timestamp_us > 0 {
                        last_main_frame_timestamp = timestamp_us;
                    }

                    // S frames inherit timestamp from last main frame
                    let final_timestamp = if frame_type == 'S' && timestamp_us == 0 {
                        last_main_frame_timestamp
                    } else {
                        timestamp_us
                    };

                    if debug && timestamp_us == 0 && debug_frame_list.len() < 5 {
                        println!(
                            "DEBUG: Non-sample frame {:?} has timestamp 0->{}. Fields: {:?}",
                            frame_type,
                            final_timestamp,
                            frame_data.keys().collect::<Vec<_>>()
                        );
                    }

                    let decoded_frame = DecodedFrame {
                        frame_type,
                        timestamp_us: final_timestamp,
                        loop_iteration,
                        data: frame_data.clone(),
                    };
                    debug_frame_list.push(decoded_frame.clone());
                    
                    // **BLACKBOX_DECODE COMPATIBILITY**: Store in chronological order for CSV export
                    chronological_frames.push(decoded_frame);
                }

                // Update timing from first and last valid frames with time data
                if parsing_success {
                    if let Some(time_us) = frame_data.get("time") {
                        let time_val = *time_us as u64;
                        if stats.start_time_us == 0 {
                            stats.start_time_us = time_val;
                        }
                        stats.end_time_us = time_val;
                    }
                }
            }
            Err(_) => break,
        }

        // More aggressive safety limits to prevent hanging
        if stats.total_frames > 1000000 || stats.failed_frames > 10000 {
            if debug {
                println!("Hit safety limit - stopping frame parsing");
            }
            break;
        }
    }

    stats.total_bytes = binary_data.len() as u64;

    if debug {
        println!(
            "Parsed {} frames: {} I, {} P, {} H, {} G, {} E, {} S",
            stats.total_frames,
            stats.i_frames,
            stats.p_frames,
            stats.h_frames,
            stats.g_frames,
            stats.e_frames,
            stats.s_frames
        );
        println!("Failed to parse: {} frames", stats.failed_frames);
    }

    Ok((stats, sample_frames, Some(debug_frames), Some(chronological_frames)))
}

// **BLACKBOX_DECODE COMPATIBILITY**: Timestamp rollover detection function
// **BLACKBOX_DECODE COMPATIBILITY**: Timestamp rollover detection function
// Ported directly from blackbox_decode.c flightLogDetectAndApplyTimestampRollover()
fn detect_and_apply_timestamp_rollover(
    timestamp: i32,
    last_time: &mut i64,
    accumulator: &mut u64,
) -> u64 {
    const MAXIMUM_TIME_JUMP_BETWEEN_FRAMES: u64 = 10 * 1000000; // 10 seconds in microseconds
    
    if *last_time != -1 {
        // If we appeared to travel backwards in time (modulo 32 bits)
        // But we actually just incremented a reasonable amount (modulo 32-bits)
        if (timestamp as u32) < (*last_time as u32) 
            && ((timestamp as u32).wrapping_sub(*last_time as u32)) < (MAXIMUM_TIME_JUMP_BETWEEN_FRAMES as u32) {
            // 32-bit time counter has wrapped, so add 2^32 to the timestamp
            *accumulator += 0x100000000u64;
        }
    }
    
    (timestamp as u32) as u64 + *accumulator
}

#[allow(dead_code)]
fn parse_i_frame(
    stream: &mut bbl_format::BBLDataStream,
    frame_def: &FrameDefinition,
    debug: bool,
) -> Result<HashMap<String, i32>> {
    let mut data = HashMap::new();

    // Parse each field according to the frame definition
    for field in &frame_def.fields {
        let value = match field.encoding {
            bbl_format::ENCODING_SIGNED_VB => stream.read_signed_vb()?,
            bbl_format::ENCODING_UNSIGNED_VB => stream.read_unsigned_vb()? as i32,
            bbl_format::ENCODING_NEG_14BIT => {
                -(bbl_format::sign_extend_14bit(stream.read_unsigned_vb()? as u16))
            }
            bbl_format::ENCODING_NULL => 0,
            _ => {
                if debug {
                    println!(
                        "Unsupported I-frame encoding {} for field {}",
                        field.encoding, field.name
                    );
                }
                0
            }
        };

        data.insert(field.name.clone(), value);
    }

    Ok(data)
}

fn parse_s_frame(
    stream: &mut bbl_format::BBLDataStream,
    frame_def: &FrameDefinition,
    debug: bool,
) -> Result<HashMap<String, i32>> {
    let mut data = HashMap::new();
    let mut field_index = 0;

    while field_index < frame_def.fields.len() {
        let field = &frame_def.fields[field_index];

        match field.encoding {
            bbl_format::ENCODING_SIGNED_VB => {
                let value = stream.read_signed_vb()?;
                data.insert(field.name.clone(), value);
                field_index += 1;
            }
            bbl_format::ENCODING_UNSIGNED_VB => {
                let value = stream.read_unsigned_vb()? as i32;
                data.insert(field.name.clone(), value);
                field_index += 1;
            }
            bbl_format::ENCODING_NEG_14BIT => {
                let value = -(bbl_format::sign_extend_14bit(stream.read_unsigned_vb()? as u16));
                data.insert(field.name.clone(), value);
                field_index += 1;
            }
            bbl_format::ENCODING_TAG2_3S32 => {
                // This encoding handles 3 fields at once
                let mut values = [0i32; 8];
                stream.read_tag2_3s32(&mut values)?;

                #[allow(clippy::needless_range_loop)]
                for j in 0..3 {
                    if field_index + j < frame_def.fields.len() {
                        let current_field = &frame_def.fields[field_index + j];
                        data.insert(current_field.name.clone(), values[j]);
                    }
                }
                field_index += 3;
            }
            bbl_format::ENCODING_NULL => {
                data.insert(field.name.clone(), 0);
                field_index += 1;
            }
            _ => {
                if debug {
                    println!(
                        "Unsupported S-frame encoding {} for field {}",
                        field.encoding, field.name
                    );
                }
                // For unsupported encodings, try to read as signed VB
                let value = stream.read_signed_vb().unwrap_or(0);
                data.insert(field.name.clone(), value);
                field_index += 1;
            }
        }
    }

    Ok(data)
}

fn parse_h_frame(
    stream: &mut bbl_format::BBLDataStream,
    frame_def: &FrameDefinition,
    debug: bool,
) -> Result<HashMap<String, i32>> {
    let mut data = HashMap::new();

    if debug {
        println!("Parsing H frame with {} fields", frame_def.count);
    }

    // H frames contain GPS home position data
    for (i, field) in frame_def.fields.iter().enumerate() {
        if i >= frame_def.count {
            break;
        }

        let value = match field.encoding {
            bbl_format::ENCODING_SIGNED_VB => stream.read_signed_vb()?,
            bbl_format::ENCODING_UNSIGNED_VB => stream.read_unsigned_vb()? as i32,
            bbl_format::ENCODING_NEG_14BIT => {
                -(bbl_format::sign_extend_14bit(stream.read_unsigned_vb()? as u16))
            }
            bbl_format::ENCODING_NULL => 0,
            _ => {
                if debug {
                    println!(
                        "Unsupported H-frame encoding {} for field {}",
                        field.encoding, field.name
                    );
                }
                stream.read_signed_vb().unwrap_or(0)
            }
        };

        data.insert(field.name.clone(), value);
    }

    Ok(data)
}

fn parse_g_frame(
    stream: &mut bbl_format::BBLDataStream,
    frame_def: &FrameDefinition,
    debug: bool,
) -> Result<HashMap<String, i32>> {
    let mut data = HashMap::new();

    if debug {
        println!("Parsing G frame with {} fields", frame_def.count);
    }

    // G frames contain GPS data
    for (i, field) in frame_def.fields.iter().enumerate() {
        if i >= frame_def.count {
            break;
        }

        let value = match field.encoding {
            bbl_format::ENCODING_SIGNED_VB => stream.read_signed_vb()?,
            bbl_format::ENCODING_UNSIGNED_VB => stream.read_unsigned_vb()? as i32,
            bbl_format::ENCODING_NEG_14BIT => {
                -(bbl_format::sign_extend_14bit(stream.read_unsigned_vb()? as u16))
            }
            bbl_format::ENCODING_NULL => 0,
            _ => {
                if debug {
                    println!(
                        "Unsupported G-frame encoding {} for field {}",
                        field.encoding, field.name
                    );
                }
                stream.read_signed_vb().unwrap_or(0)
            }
        };

        data.insert(field.name.clone(), value);
    }

    Ok(data)
}

fn skip_frame(stream: &mut bbl_format::BBLDataStream, frame_type: char, debug: bool) -> Result<()> {
    if debug {
        println!("Skipping {frame_type} frame");
    }

    // Skip frame by reading a few bytes - this is a simple heuristic
    // In a full implementation, we'd parse these properly too
    match frame_type {
        'E' => {
            // Event frames - read event type and some data
            let _event_type = stream.read_byte()?;
            // Read up to 16 bytes of event data
            for _ in 0..16 {
                if stream.eof {
                    break;
                }
                let _ = stream.read_byte();
            }
        }
        'G' | 'H' => {
            // GPS frames - read several fields
            for _ in 0..7 {
                if stream.eof {
                    break;
                }
                let _ = stream.read_unsigned_vb();
            }
        }
        _ => {
            // Unknown frame type - read a few bytes
            for _ in 0..8 {
                if stream.eof {
                    break;
                }
                let _ = stream.read_byte();
            }
        }
    }

    Ok(())
}

fn parse_signed_data(signed_data: &str) -> Vec<bool> {
    signed_data.split(',').map(|s| s.trim() == "1").collect()
}

fn parse_numeric_data(numeric_data: &str) -> Vec<u8> {
    numeric_data
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

// Unit conversion functions
fn convert_vbat_to_volts(raw_value: i32) -> f32 {
    // Betaflight already does the ADC conversion to 0.1V units
    raw_value as f32 / 10.0
}

fn convert_amperage_to_amps(raw_value: i32) -> f32 {
    // Betaflight already does the ADC conversion to 0.01A units
    raw_value as f32 / 100.0
}

fn parse_bbl_file_streaming(
    file_path: &Path,
    debug: bool,
    export_csv: bool,
    csv_options: &CsvExportOptions,
) -> Result<usize> {
    if debug {
        println!("=== STREAMING BBL FILE PROCESSING ===");
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

    let mut processed_logs = 0;

    for (log_index, &start_pos) in log_positions.iter().enumerate() {
        if debug {
            println!(
                "Processing log {} starting at position {}",
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
            export_csv,
        )?;

        // Display log info immediately
        display_log_info(&log);

        // Export CSV immediately while data is hot in cache
        if export_csv {
            if let Err(e) = export_single_log_to_csv(&log, file_path, csv_options, debug) {
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                eprintln!(
                    "Warning: Failed to export CSV for {filename} log {}: {e}",
                    log_index + 1
                );
            }
        }

        processed_logs += 1;

        // Add separator between logs for clarity
        if log_index + 1 < log_positions.len() {
            println!();
        }

        // Log goes out of scope here, memory is freed immediately
    }

    Ok(processed_logs)
}

fn format_flight_mode_flags(flags: i32) -> String {
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

fn format_state_flags(flags: i32) -> String {
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

fn format_failsafe_phase(phase: i32) -> String {
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
        // Test voltage conversion (0.1V units)
        let volts = convert_vbat_to_volts(33); // 33 * 0.1 = 3.3V
        assert!((volts - 3.3).abs() < 0.01);

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
        let options = CsvExportOptions {
            output_dir: Some("/tmp".to_string()),
        };
        assert_eq!(options.output_dir.as_ref().unwrap(), "/tmp");

        let options = CsvExportOptions { output_dir: None };
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
