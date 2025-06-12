mod bbl_format;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use glob::glob;
use std::collections::HashMap;
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
        let fields = names.iter().map(|name| FieldDefinition {
            name: name.clone(),
            signed: false,
            predictor: 0,
            encoding: 0,
        }).collect();
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

#[derive(Debug)]
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

impl Default for FrameStats {
    fn default() -> Self {
        Self {
            i_frames: 0,
            p_frames: 0,
            h_frames: 0,
            g_frames: 0,
            e_frames: 0,
            s_frames: 0,
            total_frames: 0,
            total_bytes: 0,
            start_time_us: 0,
            end_time_us: 0,
            failed_frames: 0,
            missing_iterations: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct DecodedFrame {
    frame_type: char,
    timestamp_us: u64,
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

fn main() -> Result<()> {
    let matches = Command::new("BBL Parser")
        .version("1.0")
        .about("Parse and analyze BBL blackbox log files from Betaflight, EmuFlight, INAV and other flight controllers using JavaScript reference implementation")
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
                .help("Export decoded frame data to CSV files")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let debug = matches.get_flag("debug");
    let export_csv = matches.get_flag("csv");
    let file_patterns: Vec<&String> = matches.get_many::<String>("files").unwrap().collect();
    
    let mut processed_files = 0;

    // Collect all valid file paths
    let mut valid_paths = Vec::new();
    for pattern in &file_patterns {
        let paths: Vec<_> = if pattern.contains('*') || pattern.contains('?') {
            glob(pattern)
                .with_context(|| format!("Invalid glob pattern: {}", pattern))?
                .collect::<Result<Vec<_>, _>>()
                .with_context(|| format!("Error expanding glob pattern: {}", pattern))?
        } else {
            vec![Path::new(pattern).to_path_buf()]
        };

        for path in paths {
            if !path.exists() {
                eprintln!("Warning: File does not exist: {:?}", path);
                continue;
            }
            let valid_extension = path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| {
                    let ext_lower = ext.to_ascii_lowercase();
                    ext_lower == "bbl" || ext_lower == "bfl" || ext_lower == "txt"
                })
                .unwrap_or(false);
            
            if !valid_extension {
                eprintln!("Warning: Skipping file with unsupported extension: {:?}", path);
                continue;
            }
            valid_paths.push(path);
        }
    }

    // Process files
    for (index, path) in valid_paths.iter().enumerate() {
        if index > 0 {
            println!();
        }
        
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        println!("Processing: {}", filename);
        
        let logs = parse_bbl_file(&path, debug)?;
        
        if debug {
            println!("\n=== DEBUG INFORMATION ===");
            display_debug_info(&logs);
        }
        
        for log in &logs {
            display_log_info(log);
        }
        
        if export_csv {
            export_logs_to_csv(&logs, &path)?;
        }
        
        processed_files += 1;
    }

    if processed_files == 0 {
        eprintln!("No files were successfully processed.");
        std::process::exit(1);
    }

    Ok(())
}

fn parse_bbl_file(file_path: &Path, debug: bool) -> Result<Vec<BBLLog>> {
    if debug {
        println!("=== PARSING BBL FILE ===");
        let metadata = std::fs::metadata(file_path)?;
        println!("File size: {} bytes ({:.2} MB)", metadata.len(), metadata.len() as f64 / 1024.0 / 1024.0);
    }
    
    let file_data = std::fs::read(file_path)?;
    
    // Look for multiple logs by searching for log start markers
    let log_start_marker = b"H Product:Blackbox flight data recorder by Nicholas Sherlock";
    let mut log_positions = Vec::new();
    
    // Find all log start positions
    for i in 0..file_data.len() {
        if i + log_start_marker.len() <= file_data.len() {
            if &file_data[i..i + log_start_marker.len()] == log_start_marker {
                log_positions.push(i);
            }
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
            println!("Parsing log {} starting at position {}", log_index + 1, start_pos);
        }
        
        // Determine end position (start of next log or end of file)
        let end_pos = log_positions.get(log_index + 1).copied().unwrap_or(file_data.len());
        let log_data = &file_data[start_pos..end_pos];
        
        // Parse this individual log
        let log = parse_single_log(log_data, log_index + 1, log_positions.len(), debug)?;
        logs.push(log);
    }
    
    Ok(logs)
}

fn parse_single_log(log_data: &[u8], log_number: usize, total_logs: usize, debug: bool) -> Result<BBLLog> {
    // Find where headers end and binary data begins
    let mut header_end = 0;
    for i in 1..log_data.len() {
        if log_data[i-1] == b'\n' && log_data[i] != b'H' {
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
    let (mut stats, frames, debug_frames) = parse_frames(binary_data, &header, debug)?;
    
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
            firmware_revision = line.strip_prefix("H Firmware revision:").unwrap_or("").trim().to_string();
        } else if line.starts_with("H Board information:") {
            board_info = line.strip_prefix("H Board information:").unwrap_or("").trim().to_string();
        } else if line.starts_with("H Craft name:") {
            craft_name = line.strip_prefix("H Craft name:").unwrap_or("").trim().to_string();
        } else if line.starts_with("H Data version:") {
            if let Ok(version) = line.strip_prefix("H Data version:").unwrap_or("2").trim().parse() {
                data_version = version;
            }
        } else if line.starts_with("H looptime:") {
            if let Ok(lt) = line.strip_prefix("H looptime:").unwrap_or("0").trim().parse() {
                looptime = lt;
            }
        } else if line.starts_with("H Field I name:") {
            // Parse I frame field names
            if let Some(field_str) = line.strip_prefix("H Field I name:") {
                let names: Vec<String> = field_str.split(',').map(|s| s.trim().to_string()).collect();
                i_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field P name:") {
            // Parse P frame field names
            if let Some(field_str) = line.strip_prefix("H Field P name:") {
                let names: Vec<String> = field_str.split(',').map(|s| s.trim().to_string()).collect();
                p_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field S name:") {
            // Parse S frame field names
            if let Some(field_str) = line.strip_prefix("H Field S name:") {
                let names: Vec<String> = field_str.split(',').map(|s| s.trim().to_string()).collect();
                s_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field G name:") {
            // Parse G frame field names
            if let Some(field_str) = line.strip_prefix("H Field G name:") {
                let names: Vec<String> = field_str.split(',').map(|s| s.trim().to_string()).collect();
                g_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field H name:") {
            // Parse H frame field names
            if let Some(field_str) = line.strip_prefix("H Field H name:") {
                let names: Vec<String> = field_str.split(',').map(|s| s.trim().to_string()).collect();
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
                    p_frame_def = FrameDefinition::from_field_names(i_frame_def.field_names.clone());
                }
                p_frame_def.update_predictors(&predictors);
            }
        } else if line.starts_with("H Field P encoding:") {
            // Parse P frame encodings
            if let Some(enc_str) = line.strip_prefix("H Field P encoding:") {
                let encodings = parse_numeric_data(enc_str);
                // P frames inherit field names from I frames but have their own encodings
                if p_frame_def.field_names.is_empty() && !i_frame_def.field_names.is_empty() {
                    p_frame_def = FrameDefinition::from_field_names(i_frame_def.field_names.clone());
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
        println!("Parsed headers: Firmware={}, Board={}, Craft={}", 
                 firmware_revision, board_info, craft_name);
        println!("Data version: {}, Looptime: {}", data_version, looptime);
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
                        field_names.iter().filter(|name| 
                            name.as_str() != "time" && name.as_str() != "loopIteration"
                        ).copied().collect()
                    };
                    
                    // Print header
                    print!("  {:>8} {:>12} {:>8}", "Index", "Time(μs)", "Loop");
                    for field_name in &selected_fields {
                        print!(" {:>10}", if field_name.len() > 10 { 
                            &field_name[..10] 
                        } else { 
                            field_name 
                        });
                    }
                    if field_names.len() > max_fields_to_show {
                        print!(" ... ({} more fields)", field_names.len() - selected_fields.len() - 2);
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
                        indices.extend((mid-2)..(mid+3));
                        // Last 5
                        indices.extend((frames.len()-5)..frames.len());
                        indices
                    };
                    
                    let mut last_shown_index = None;
                    for &index in &frames_to_show {
                        // Show ellipsis if there's a gap
                        if let Some(last_idx) = last_shown_index {
                            if index > last_idx + 1 {
                                println!("  {:>8} {:>12} {:>8} ... ({} frames skipped)", 
                                        "...", "...", "...", index - last_idx - 1);
                            }
                        }
                        
                        let frame = &frames[index];
                        print!("  {:>8} {:>12} {:>8}", 
                               index, frame.timestamp_us, frame.loop_iteration);
                        
                        for field_name in &selected_fields {
                            let value = frame.data.get(*field_name).copied().unwrap_or(0);
                            print!(" {:>10}", value);
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

fn display_debug_info(logs: &[BBLLog]) {
    if let Some(log) = logs.first() {
        println!("\n=== BBL FILE HEADERS ===");
        println!("Total headers: {}", log.header.all_headers.len());
        
        // Show key configuration
        println!("\nKey Configuration:");
        for header in &log.header.all_headers {
            if header.contains("Firmware revision:") ||
               header.contains("Board information:") ||
               header.contains("Craft name:") ||
               header.contains("looptime:") {
                println!("{}", header);
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
    
    println!("\nLog {} of {}, frames: {}", 
             log.log_number, log.total_logs, stats.total_frames);
    
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
    
    // Display timing if available
    if stats.start_time_us > 0 && stats.end_time_us > stats.start_time_us {
        let duration_ms = (stats.end_time_us.saturating_sub(stats.start_time_us)) / 1000;
        println!("Duration   {:6} ms", duration_ms);
    }
    
    // Display data version and missing iterations
    if header.data_version > 0 {
        println!("Data ver   {:6}", header.data_version);
    }
    if stats.missing_iterations > 0 {
        println!("Missing    {:6} iterations", stats.missing_iterations);
    }
}

fn export_logs_to_csv(logs: &[BBLLog], bbl_path: &Path) -> Result<()> {
    let base_name = bbl_path.file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    
    for log in logs {
        if log.sample_frames.is_empty() {
            println!("Log {} has no sample frames to show", log.log_number);
            continue;
        }
        
        let csv_filename = if log.total_logs > 1 {
            format!("{}.{:02}.csv", base_name, log.log_number)
        } else {
            format!("{}.csv", base_name)
        };
        
        println!("Would export log {} to: {} ({} total frames with {} frame types)", 
                log.log_number, csv_filename, log.stats.total_frames,
                log.sample_frames.iter().map(|f| f.frame_type).collect::<std::collections::HashSet<_>>().len());
        
        // Show sample of frame data from the stored samples
        if let Some(first_frame) = log.sample_frames.first() {
            println!("  Sample frame: type={}, time={}μs, iteration={}, fields={}", 
                    first_frame.frame_type, first_frame.timestamp_us, 
                    first_frame.loop_iteration, first_frame.data.len());
        }
    }
    
    Ok(())
}

fn parse_frames(binary_data: &[u8], header: &BBLHeader, debug: bool) -> Result<(FrameStats, Vec<DecodedFrame>, Option<HashMap<char, Vec<DecodedFrame>>>)> {
    let mut stats = FrameStats::default();
    let mut sample_frames = Vec::new();
    let mut debug_frames: Option<HashMap<char, Vec<DecodedFrame>>> = if debug {
        Some(HashMap::new())
    } else {
        None
    };
    
    if debug {
        println!("Binary data size: {} bytes", binary_data.len());
        if !binary_data.is_empty() {
            println!("First 16 bytes: {:02X?}", &binary_data[..16.min(binary_data.len())]);
        }
    }
    
    if binary_data.is_empty() {
        return Ok((stats, sample_frames, debug_frames));
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
                            println!("Unknown frame type byte 0x{:02X} ('{:?}') at offset {}", 
                                   frame_type_byte, frame_type_byte as char, frame_start_pos);
                        }
                        stats.failed_frames += 1;
                        continue;
                    }
                };
                
                if debug && stats.total_frames < 3 {
                    println!("Found frame type '{}' at offset {}", frame_type, frame_start_pos);
                }
                
                // Parse frame using proper streaming logic
                let mut frame_data = HashMap::new();
                let mut parsing_success = false;
                
                match frame_type {
                    'I' => {
                        if header.i_frame_def.count > 0 {
                            // I-frames reset the prediction history
                            frame_history.current_frame.fill(0);
                            
                            if let Ok(_) = bbl_format::parse_frame_data(
                                &mut stream,
                                &header.i_frame_def,
                                &mut frame_history.current_frame,
                                None, // I-frames don't use prediction
                                None,
                                0,
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                            ) {
                                // Copy parsed data to frame_data HashMap
                                for (i, field_name) in header.i_frame_def.field_names.iter().enumerate() {
                                    if i < frame_history.current_frame.len() {
                                        frame_data.insert(field_name.clone(), frame_history.current_frame[i]);
                                    }
                                }
                                
                                // Update history for future P-frames
                                frame_history.previous2_frame.copy_from_slice(&frame_history.previous_frame);
                                frame_history.previous_frame.copy_from_slice(&frame_history.current_frame);
                                frame_history.valid = true;
                                parsing_success = true;
                                stats.i_frames += 1;
                            }
                        }
                    },
                    'P' => {
                        if header.p_frame_def.count > 0 && frame_history.valid {
                            frame_history.current_frame.fill(0);
                            
                            if let Ok(_) = bbl_format::parse_frame_data(
                                &mut stream,
                                &header.p_frame_def,
                                &mut frame_history.current_frame,
                                Some(&frame_history.previous_frame),
                                Some(&frame_history.previous2_frame),
                                0, // TODO: Calculate skipped frames properly
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                            ) {
                                // Copy parsed data using I-frame field names (P-frames use I-frame structure)
                                for (i, field_name) in header.i_frame_def.field_names.iter().enumerate() {
                                    if i < frame_history.current_frame.len() {
                                        frame_data.insert(field_name.clone(), frame_history.current_frame[i]);
                                    }
                                }
                                
                                // Update history
                                frame_history.previous2_frame.copy_from_slice(&frame_history.previous_frame);
                                frame_history.previous_frame.copy_from_slice(&frame_history.current_frame);
                                parsing_success = true;
                                stats.p_frames += 1;
                            }
                        } else {
                            // Skip P-frame if we don't have valid I-frame history
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.failed_frames += 1;
                        }
                    },
                    'S' => {
                        if header.s_frame_def.count > 0 {
                            if let Ok(data) = parse_s_frame(&mut stream, &header.s_frame_def, debug) {
                                frame_data = data;
                                parsing_success = true;
                                stats.s_frames += 1;
                            }
                        }
                    },
                    'G' | 'H' | 'E' => {
                        skip_frame(&mut stream, frame_type, debug)?;
                        match frame_type {
                            'G' => stats.g_frames += 1,
                            'H' => stats.h_frames += 1,
                            'E' => stats.e_frames += 1,
                            _ => {}
                        }
                        parsing_success = true;
                    },
                    _ => {}
                };
                
                if !parsing_success {
                    stats.failed_frames += 1;
                }
                
                stats.total_frames += 1;
                
                // Show progress for large files  
                if debug && stats.total_frames % 50000 == 0 {
                    println!("Parsed {} frames so far...", stats.total_frames);
                } else if stats.total_frames % 100000 == 0 {
                    println!("Parsed {} frames so far...", stats.total_frames);
                }
                
                // Store only a few sample frames for display purposes
                if parsing_success && sample_frames.len() < 10 {
                    // Extract timing before moving frame_data
                    let timestamp_us = frame_data.get("time").copied().unwrap_or(0) as u64;
                    let loop_iteration = frame_data.get("loopIteration").copied().unwrap_or(0) as u32;
                    
                    let decoded_frame = DecodedFrame {
                        frame_type,
                        timestamp_us,
                        loop_iteration,
                        data: frame_data.clone(),
                    };
                    sample_frames.push(decoded_frame.clone());
                    
                    // Store debug frames if debug mode is enabled
                    if let Some(ref mut debug_map) = debug_frames {
                        let debug_frame_list = debug_map.entry(frame_type).or_insert_with(Vec::new);
                        debug_frame_list.push(decoded_frame);
                    }
                } else if parsing_success {
                    // Even if we don't store in sample_frames, still store for debug if enabled
                    if let Some(ref mut debug_map) = debug_frames {
                        let debug_frame_list = debug_map.entry(frame_type).or_insert_with(Vec::new);
                        // Store frames strategically for the display pattern (first/middle/last)
                        if debug_frame_list.len() < 50 {
                            let timestamp_us = frame_data.get("time").copied().unwrap_or(0) as u64;
                            let loop_iteration = frame_data.get("loopIteration").copied().unwrap_or(0) as u32;
                            
                            let decoded_frame = DecodedFrame {
                                frame_type,
                                timestamp_us,
                                loop_iteration,
                                data: frame_data.clone(),
                            };
                            debug_frame_list.push(decoded_frame);
                        }
                    }
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
        println!("Parsed {} frames: {} I, {} P, {} H, {} G, {} E, {} S",
                 stats.total_frames, stats.i_frames, stats.p_frames,
                 stats.h_frames, stats.g_frames, stats.e_frames, stats.s_frames);
        println!("Failed to parse: {} frames", stats.failed_frames);
    }
    
    Ok((stats, sample_frames, debug_frames))
}

#[allow(dead_code)]
fn parse_i_frame(stream: &mut bbl_format::BBLDataStream, frame_def: &FrameDefinition, debug: bool) -> Result<HashMap<String, i32>> {
    let mut data = HashMap::new();
    
    // Parse each field according to the frame definition
    for field in &frame_def.fields {
        let value = match field.encoding {
            bbl_format::ENCODING_SIGNED_VB => stream.read_signed_vb()?,
            bbl_format::ENCODING_UNSIGNED_VB => stream.read_unsigned_vb()? as i32,
            bbl_format::ENCODING_NEG_14BIT => -(bbl_format::sign_extend_14bit(stream.read_unsigned_vb()? as u16) as i32),
            bbl_format::ENCODING_NULL => 0,
            _ => {
                if debug {
                    println!("Unsupported I-frame encoding {} for field {}", field.encoding, field.name);
                }
                0
            }
        };
        
        data.insert(field.name.clone(), value);
    }
    
    Ok(data)
}

fn parse_s_frame(stream: &mut bbl_format::BBLDataStream, frame_def: &FrameDefinition, debug: bool) -> Result<HashMap<String, i32>> {
    let mut data = HashMap::new();
    
    // Parse each field according to the frame definition
    for field in &frame_def.fields {
        let value = match field.encoding {
            bbl_format::ENCODING_SIGNED_VB => stream.read_signed_vb()?,
            bbl_format::ENCODING_UNSIGNED_VB => stream.read_unsigned_vb()? as i32,
            bbl_format::ENCODING_NEG_14BIT => -(bbl_format::sign_extend_14bit(stream.read_unsigned_vb()? as u16) as i32),
            bbl_format::ENCODING_NULL => 0,
            _ => {
                if debug {
                    println!("Unsupported S-frame encoding {} for field {}", field.encoding, field.name);
                }
                // For unsupported encodings, try to read as signed VB
                stream.read_signed_vb().unwrap_or(0)
            }
        };
        
        data.insert(field.name.clone(), value);
    }
    
    Ok(data)
}

fn skip_frame(stream: &mut bbl_format::BBLDataStream, frame_type: char, debug: bool) -> Result<()> {
    if debug {
        println!("Skipping {} frame", frame_type);
    }
    
    // Skip frame by reading a few bytes - this is a simple heuristic
    // In a full implementation, we'd parse these properly too
    match frame_type {
        'E' => {
            // Event frames - read event type and some data
            let _event_type = stream.read_byte()?;
            // Read up to 16 bytes of event data
            for _ in 0..16 {
                if stream.eof { break; }
                let _ = stream.read_byte();
            }
        },
        'G' | 'H' => {
            // GPS frames - read several fields
            for _ in 0..7 {
                if stream.eof { break; }
                let _ = stream.read_unsigned_vb();
            }
        },
        _ => {
            // Unknown frame type - read a few bytes
            for _ in 0..8 {
                if stream.eof { break; }
                let _ = stream.read_byte();
            }
        }
    }
    
    Ok(())
}

fn parse_signed_data(signed_data: &str) -> Vec<bool> {
    signed_data.split(',')
        .map(|s| s.trim() == "1")
        .collect()
}

fn parse_numeric_data(numeric_data: &str) -> Vec<u8> {
    numeric_data.split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}
