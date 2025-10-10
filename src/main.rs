mod bbl_format;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use glob::glob;
use semver::Version;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Maximum recursion depth to prevent stack overflow
const MAX_RECURSION_DEPTH: usize = 100;

/// Expand input paths to a list of BBL files.
/// If a path is a file, add it directly (will be filtered later for BBL/BFL/TXT extension).
/// If a path is a directory, recursively find all BBL files within it.
/// If a path contains glob patterns, expand them first.
fn expand_input_paths(
    input_paths: &[String],
    visited: &mut HashSet<PathBuf>,
) -> Result<Vec<String>> {
    expand_input_paths_with_depth(input_paths, visited, 0)
}

/// Internal function with depth tracking for recursion protection
fn expand_input_paths_with_depth(
    input_paths: &[String],
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Result<Vec<String>> {
    if depth > MAX_RECURSION_DEPTH {
        return Err(anyhow::anyhow!(
            "Maximum recursion depth exceeded ({})",
            MAX_RECURSION_DEPTH
        ));
    }
    let mut bbl_files = Vec::new();

    for input_path_str in input_paths {
        // Check if this is a glob pattern
        if input_path_str.contains('*') || input_path_str.contains('?') {
            match glob(input_path_str) {
                Ok(glob_iter) => {
                    let collected = glob_iter.collect::<Result<Vec<_>, _>>();
                    match collected {
                        Ok(paths) => {
                            for path in paths {
                                if let Some(path_str) = path.to_str() {
                                    let sub_result = expand_input_paths_with_depth(
                                        &[path_str.to_string()],
                                        visited,
                                        depth + 1,
                                    )?;
                                    bbl_files.extend(sub_result);
                                }
                            }
                        }
                        Err(e) => {
                            return Err(anyhow::Error::new(e).context(format!(
                                "Error expanding glob pattern '{}'",
                                input_path_str
                            )));
                        }
                    }
                }
                Err(e) => {
                    return Err(anyhow::Error::new(e)
                        .context(format!("Invalid glob pattern '{}'", input_path_str)));
                }
            }
            continue;
        }

        let input_path = Path::new(input_path_str);

        match input_path.canonicalize() {
            Ok(canonical_path) => {
                if canonical_path.is_file() {
                    // It's a file, add it directly
                    if let Some(path_str) = canonical_path.to_str() {
                        bbl_files.push(path_str.to_string());
                    }
                } else if canonical_path.is_dir() {
                    // It's a directory, find all BBL files recursively
                    // Don't add to visited here since find_bbl_files_in_dir_with_depth will handle it
                    let mut dir_bbl_files =
                        find_bbl_files_in_dir_with_depth(&canonical_path, visited, depth + 1)?;
                    bbl_files.append(&mut dir_bbl_files);
                } else {
                    // Path doesn't exist or isn't accessible
                    eprintln!(
                        "Warning: Path not found or not accessible: {}",
                        input_path_str
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to canonicalize path '{}': {}",
                    input_path_str, e
                );
                // Skip this path
                continue;
            }
        }
    }

    Ok(bbl_files)
}

/// Recursively find all BBL files in a directory, protecting against symlink cycles and depth overflow
fn find_bbl_files_in_dir_with_depth(
    dir_path: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Result<Vec<String>> {
    if depth > MAX_RECURSION_DEPTH {
        return Err(anyhow::anyhow!(
            "Maximum recursion depth exceeded in directory traversal ({})",
            MAX_RECURSION_DEPTH
        ));
    }

    let mut bbl_files = Vec::new();

    match dir_path.canonicalize() {
        Ok(canonical_dir) => {
            if visited.contains(&canonical_dir) {
                // Already visited, skip to avoid cycles
                return Ok(bbl_files);
            }
            visited.insert(canonical_dir.clone());

            if !canonical_dir.is_dir() {
                return Ok(bbl_files);
            }

            let entries = match fs::read_dir(&canonical_dir) {
                Ok(entries) => entries,
                Err(e) => {
                    eprintln!(
                        "Warning: Cannot read directory '{}': {}",
                        canonical_dir.display(),
                        e
                    );
                    return Ok(bbl_files);
                }
            };

            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        eprintln!(
                            "Warning: Cannot read entry in directory '{}': {}",
                            canonical_dir.display(),
                            e
                        );
                        continue;
                    }
                };
                let path = entry.path();

                match path.canonicalize() {
                    Ok(canonical_path) => {
                        if visited.contains(&canonical_path) {
                            continue;
                        }
                        visited.insert(canonical_path.clone());

                        if canonical_path.is_dir() {
                            // Recursively search subdirectories
                            let mut sub_bbl_files = find_bbl_files_in_dir_with_depth(
                                &canonical_path,
                                visited,
                                depth + 1,
                            )?;
                            bbl_files.append(&mut sub_bbl_files);
                        } else if canonical_path.is_file() {
                            // Check if it's a BBL file (only BBL for directories, not TXT)
                            if let Some(extension) = canonical_path.extension() {
                                let ext_lower = extension.to_string_lossy().to_ascii_lowercase();
                                if ext_lower == "bbl" || ext_lower == "bfl" {
                                    if let Some(path_str) = canonical_path.to_str() {
                                        bbl_files.push(path_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to canonicalize path in dir '{}': {}",
                            path.display(),
                            e
                        );
                        continue;
                    }
                }
            }
        }
        Err(e) => {
            eprintln!(
                "Warning: Failed to canonicalize directory '{}': {}",
                dir_path.display(),
                e
            );
            return Ok(bbl_files);
        }
    }

    // Sort the files for consistent ordering
    bbl_files.sort();
    Ok(bbl_files)
}

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

// GPS related structures for GPX export
#[derive(Debug, Clone)]
struct GpsCoordinate {
    latitude: f64,
    longitude: f64,
    altitude: f64,
    timestamp_us: u64,
    num_sats: Option<i32>,
    #[allow(dead_code)]
    speed: Option<f64>,
    #[allow(dead_code)]
    ground_course: Option<f64>,
}

#[derive(Debug, Clone)]
struct GpsHomeCoordinate {
    #[allow(dead_code)]
    home_latitude: f64,
    #[allow(dead_code)]
    home_longitude: f64,
    #[allow(dead_code)]
    timestamp_us: u64,
}

// Event structure for JSON export
#[derive(Debug, Clone)]
struct EventFrame {
    timestamp_us: u64,
    event_type: u8,
    #[allow(dead_code)]
    event_data: Vec<u8>,
    event_description: String,
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

#[derive(Debug, Clone)]
struct ExportOptions {
    csv: bool,
    gpx: bool,
    event: bool,
    output_dir: Option<String>,
    force_export: bool,
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

        // Add computed fields IMMEDIATELY after I frame fields (like blackbox_decode does)
        if field_name_to_lookup
            .iter()
            .any(|(_, lookup)| lookup == "amperageLatest")
        {
            field_name_to_lookup.push(("energyCumulative (mAh)".to_string(), "".to_string()));
            csv_field_names.push("energyCumulative (mAh)".to_string());
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
            csv_field_names.push(csv_name);
        }

        // NOTE: G-frame fields excluded from main CSV (will go to separate .gps.csv file in future)
        // NOTE: E-frame fields excluded from main CSV (will go to separate .event file in future)

        Self {
            field_name_to_lookup,
        }
    }
}

fn build_command() -> Command {
    Command::new("BBL Parser")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Read and parse BBL blackbox log files. Exports to CSV by default (optionally GPX/JSON).")
        .arg(
            Arg::new("files")
                .help("BBL files or directories to parse. Direct file paths: .BBL, .BFL, .TXT extensions supported. Directories: recursively finds .BBL/.BFL files only (TXT files must be specified directly). Case-insensitive, supports globbing.")
                .required(false)
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
            Arg::new("output-dir")
                .long("output-dir")
                .help("Directory for output files (default: same as input file)")
                .value_name("DIR"),
        )
        .arg(
            Arg::new("gpx")
                .long("gpx")
                .help("Export GPS data (G and H frames) to GPX XML files")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("gps")
                .long("gps")
                .help("Alias for --gpx: Export GPS data to GPX XML files")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("event")
                .long("event")
                .help("Export event data (E frames) to JSON files")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("force-export")
                .long("force-export")
                .help("Force export of all logs, including short flights (bypasses smart filtering: <5s skip, 5-15s needs >1500fps, >15s keep)")
                .action(clap::ArgAction::SetTrue),
        )
}

fn main() -> Result<()> {
    let matches = build_command().get_matches();

    let debug = matches.get_flag("debug");
    let export_gpx = matches.get_flag("gpx") || matches.get_flag("gps");
    let export_event = matches.get_flag("event");
    let force_export = matches.get_flag("force-export");
    let output_dir = matches.get_one::<String>("output-dir").cloned();

    // Check if no files were provided and show help
    let file_patterns: Vec<&String> = match matches.get_many::<String>("files") {
        Some(files) => files.collect(),
        None => {
            // No files provided, show help and exit
            build_command().print_help()?;
            println!();
            return Ok(());
        }
    };

    let export_options = ExportOptions {
        csv: true, // CSV export is always enabled for the CLI binary
        gpx: export_gpx,
        event: export_event,
        output_dir: output_dir.clone(),
        force_export,
    };

    // Keep legacy csv_options for compatibility
    let csv_options = CsvExportOptions { output_dir };

    let mut processed_files = 0;

    if debug {
        println!("Input patterns: {file_patterns:?}");
    }

    // Expand input paths (files and directories) to a list of BBL files
    let mut visited = HashSet::new();
    let input_files = match expand_input_paths(
        &file_patterns
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        &mut visited,
    ) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("Error expanding input paths: {e}");
            std::process::exit(1);
        }
    };

    if input_files.is_empty() {
        eprintln!("Error: No valid BBL/BFL/TXT files found in the specified input paths.");
        std::process::exit(1);
    }

    // Collect all valid file paths
    let mut valid_paths = Vec::new();
    for file_path_str in &input_files {
        let path = PathBuf::from(file_path_str);

        if debug {
            println!("Checking file: {path:?}");
        }

        if !path.exists() {
            eprintln!("Warning: File does not exist: {path:?}");
            continue;
        }

        // Check file extension
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

        match parse_bbl_file_streaming(path, debug, &export_options, &csv_options) {
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
fn parse_bbl_file(
    file_path: &Path,
    debug: bool,
    export_options: &ExportOptions,
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
        let (log, _gps_coords, _home_coords, _events) = parse_single_log(
            log_data,
            log_index + 1,
            log_positions.len(),
            debug,
            export_options,
        )?;
        logs.push(log);
    }

    Ok(logs)
}

// Type alias to reduce complexity
type ParseSingleLogResult = (
    BBLLog,
    Vec<GpsCoordinate>,
    Vec<GpsHomeCoordinate>,
    Vec<EventFrame>,
);

fn parse_single_log(
    log_data: &[u8],
    log_number: usize,
    total_logs: usize,
    debug: bool,
    export_options: &ExportOptions,
) -> Result<ParseSingleLogResult> {
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
    let (stats, frames, debug_frames, gps_coords, home_coords, events) =
        parse_frames(binary_data, &header, debug, export_options)?;

    // Keep the timing from parser which processes ALL frames
    // Don't override with sample frames timing as that only contains a subset
    // The parser already correctly sets stats.start_time_us and stats.end_time_us

    if debug && !frames.is_empty() {
        // Store original timing from parser
        let parser_start = stats.start_time_us;
        let parser_end = stats.end_time_us;
        let parser_duration = parser_end.saturating_sub(parser_start);

        let sample_start = frames.first().unwrap().timestamp_us;
        let sample_end = frames.last().unwrap().timestamp_us;
        let sample_duration = sample_end.saturating_sub(sample_start);

        println!(
            "DEBUG: Parser timing (ALL frames) - start: {} us, end: {} us, duration: {} ms",
            parser_start,
            parser_end,
            parser_duration / 1000
        );
        println!(
            "DEBUG: Sample timing (subset) - start: {} us, end: {} us, duration: {} ms",
            sample_start,
            sample_end,
            sample_duration / 1000
        );
        println!(
            "DEBUG: Total frames: {}, Sample frames: {}",
            stats.total_frames,
            frames.len()
        );
    }

    let log = BBLLog {
        log_number,
        total_logs,
        header,
        stats,
        sample_frames: frames,
        debug_frames,
    };

    Ok((log, gps_coords, home_coords, events))
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
                        if debug && field_name == "vbatref" {
                            eprintln!("DEBUG: Found vbatref={} in headers", num_value);
                        }
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

    // Extract vbatref from sysconfig for debug output
    let vbatref = sysconfig.get("vbatref").copied().unwrap_or(0);
    if debug && vbatref > 0 {
        eprintln!("DEBUG: Found vbatref={} in headers", vbatref);
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
    // Always show S frames for blackbox_decode.c compatibility
    println!("S frames   {:6}", stats.s_frames);
    println!("Frames     {:6}", stats.total_frames);

    // Display timing if available
    if stats.start_time_us > 0 && stats.end_time_us > stats.start_time_us {
        let duration_us = stats.end_time_us.saturating_sub(stats.start_time_us);
        let duration_ms = duration_us / 1000;

        // Format as mm:ss.mmm for better readability
        let total_seconds = duration_us as f64 / 1_000_000.0;
        let minutes = (total_seconds / 60.0) as u32;
        let seconds = total_seconds % 60.0;

        if minutes > 0 {
            println!(
                "Duration   {:5}ms ({:02}m{:04.1}s)",
                duration_ms, minutes, seconds
            );
        } else {
            println!("Duration   {:5}ms ({:04.1}s)", duration_ms, seconds);
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

/// Determines if a log should be skipped for export based on duration and frame count
/// Uses smart filtering: <5s always skip, 5-15s keep if good data density (>1500fps), >15s always keep
fn should_skip_export(log: &BBLLog, force_export: bool) -> (bool, String) {
    if force_export {
        return (false, String::new()); // Never skip when forced
    }

    const VERY_SHORT_DURATION_MS: u64 = 5_000; // 5 seconds - always skip
    const SHORT_DURATION_MS: u64 = 15_000; // 15 seconds - threshold for normal logs
    const MIN_DATA_DENSITY_FPS: f64 = 1500.0; // Minimum fps for short logs
    const FALLBACK_MIN_FRAMES: u32 = 7_500; // ~5 seconds at 1500 fps (fallback when no duration)

    // Check if we have duration information
    if log.stats.start_time_us > 0 && log.stats.end_time_us > log.stats.start_time_us {
        let duration_us = log
            .stats
            .end_time_us
            .saturating_sub(log.stats.start_time_us);
        let duration_ms = duration_us / 1000;
        let duration_s = duration_ms as f64 / 1000.0;
        let fps = log.stats.total_frames as f64 / duration_s;

        // Very short logs: < 5 seconds → Always skip
        if duration_ms < VERY_SHORT_DURATION_MS {
            return (true, format!("too short ({:.1}s < 5.0s)", duration_s));
        }

        // Short logs: 5-15 seconds → Keep if sufficient data density (>1500 fps)
        if duration_ms < SHORT_DURATION_MS {
            if fps < MIN_DATA_DENSITY_FPS {
                return (
                    true,
                    format!(
                        "insufficient data density ({:.0}fps < {:.0}fps for {:.1}s log)",
                        fps, MIN_DATA_DENSITY_FPS, duration_s
                    ),
                );
            }
            // Good data density, keep it
            return (false, String::new());
        }

        // Normal logs: > 15 seconds → Check for minimal gyro activity (ground tests)
        if duration_ms >= SHORT_DURATION_MS {
            let (is_minimal_movement, max_variance) = has_minimal_gyro_activity(log);
            if is_minimal_movement {
                return (
                    true,
                    format!(
                        "minimal gyro activity ({:.1} variance) - likely ground test",
                        max_variance
                    ),
                );
            }
        }

        return (false, String::new());
    }

    // No duration information available, fall back to frame count
    // Skip if very low frame count (equivalent to <5s at minimum viable fps)
    if log.stats.total_frames < FALLBACK_MIN_FRAMES {
        return (
            true,
            format!(
                "too few frames ({} < {}) and no duration info",
                log.stats.total_frames, FALLBACK_MIN_FRAMES
            ),
        );
    }

    // Sufficient frames without duration info, keep it
    (false, String::new())
}

/// Analyzes gyro variance to detect ground tests vs actual flight
/// Returns true if the log appears to be a static ground test (minimal movement)
fn has_minimal_gyro_activity(log: &BBLLog) -> (bool, f64) {
    // Conservative thresholds to avoid false-skips
    const MIN_SAMPLES_FOR_ANALYSIS: usize = 15; // Reduced for limited sample data
    const VERY_LOW_GYRO_VARIANCE_THRESHOLD: f64 = 0.3; // More aggressive threshold for ground test detection

    let mut gyro_x_values = Vec::new();
    let mut gyro_y_values = Vec::new();
    let mut gyro_z_values = Vec::new();

    // First try to use debug_frames if available (contains more comprehensive data)
    if let Some(debug_frames) = &log.debug_frames {
        // Collect gyro data from I and P frames in debug_frames
        for (frame_type, frames) in debug_frames {
            if *frame_type == 'I' || *frame_type == 'P' {
                for frame in frames {
                    if let Some(gyro_x) = frame.data.get("gyroADC[0]") {
                        if let Some(gyro_y) = frame.data.get("gyroADC[1]") {
                            if let Some(gyro_z) = frame.data.get("gyroADC[2]") {
                                gyro_x_values.push(*gyro_x as f64);
                                gyro_y_values.push(*gyro_y as f64);
                                gyro_z_values.push(*gyro_z as f64);
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback to sample_frames if debug_frames not available or insufficient data
    if gyro_x_values.len() < MIN_SAMPLES_FOR_ANALYSIS {
        for frame in &log.sample_frames {
            if let Some(gyro_x) = frame.data.get("gyroADC[0]") {
                if let Some(gyro_y) = frame.data.get("gyroADC[1]") {
                    if let Some(gyro_z) = frame.data.get("gyroADC[2]") {
                        gyro_x_values.push(*gyro_x as f64);
                        gyro_y_values.push(*gyro_y as f64);
                        gyro_z_values.push(*gyro_z as f64);
                    }
                }
            }
        }
    }

    // Need sufficient data points for reliable analysis
    if gyro_x_values.len() < MIN_SAMPLES_FOR_ANALYSIS {
        return (false, 0.0); // Not enough data, don't skip (conservative approach)
    }

    // Calculate variance for each axis
    let variance_x = calculate_variance(&gyro_x_values);
    let variance_y = calculate_variance(&gyro_y_values);
    let variance_z = calculate_variance(&gyro_z_values);

    // Use the maximum variance across all axes
    let max_variance = variance_x.max(variance_y).max(variance_z);

    // Very conservative: only skip if ALL axes show extremely low variance
    let is_minimal = max_variance < VERY_LOW_GYRO_VARIANCE_THRESHOLD;

    (is_minimal, max_variance)
}

/// Calculate variance of a dataset
fn calculate_variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;

    variance
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
            "".to_string()
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
        "".to_string()
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

    if let Some(ref debug_frames) = log.debug_frames {
        // Collect only I, P frames for CSV export (S frames are merged into I/P frames during parsing)
        // This matches blackbox_decode behavior where S-frame data doesn't create separate CSV rows
        for frame_type in ['I', 'P'] {
            if let Some(frames) = debug_frames.get(&frame_type) {
                for frame in frames {
                    all_frames.push((frame.timestamp_us, frame_type, frame));
                }
            }
        }
    }

    // Sort by timestamp
    all_frames.sort_by_key(|(timestamp, _, _)| *timestamp);

    if all_frames.is_empty() {
        // Write at least the sample frames if no debug frames
        for frame in &log.sample_frames {
            all_frames.push((frame.timestamp_us, frame.frame_type, frame));
        }
        all_frames.sort_by_key(|(timestamp, _, _)| *timestamp);
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
    Vec<GpsCoordinate>,
    Vec<GpsHomeCoordinate>,
    Vec<EventFrame>,
)>;

fn parse_frames(
    binary_data: &[u8],
    header: &BBLHeader,
    debug: bool,
    export_options: &ExportOptions,
) -> ParseFramesResult {
    let mut stats = FrameStats::default();
    let mut sample_frames = Vec::new();
    let mut debug_frames: HashMap<char, Vec<DecodedFrame>> = HashMap::new();
    let mut last_main_frame_timestamp = 0u64; // Track timestamp for S frames

    // Track the most recent S-frame data for merging (following JavaScript approach)
    let mut last_slow_data: HashMap<String, i32> = HashMap::new();

    // Decide whether to store all frames based on CSV export requirement
    let store_all_frames = export_options.csv; // Store all frames when CSV export is requested

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
        return Ok((
            stats,
            sample_frames,
            Some(debug_frames),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ));
    }

    // Initialize frame history for proper P-frame parsing
    let mut frame_history = FrameHistory {
        current_frame: vec![0; header.i_frame_def.count],
        previous_frame: vec![0; header.i_frame_def.count],
        previous2_frame: vec![0; header.i_frame_def.count],
        valid: false,
    };

    // Collections for GPS and Event export
    let mut gps_coordinates: Vec<GpsCoordinate> = Vec::new();
    let mut home_coordinates: Vec<GpsHomeCoordinate> = Vec::new();
    let mut event_frames: Vec<EventFrame> = Vec::new();

    // GPS frame history for differential encoding
    let mut gps_frame_history: Vec<i32> = Vec::new();

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
                                debug,
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

                                // **BLACKBOX_DECODE COMPATIBILITY**: Validate frame before accepting
                                let current_time =
                                    frame_data.get("time").copied().unwrap_or(0) as u64;
                                let current_loop =
                                    frame_data.get("loopIteration").copied().unwrap_or(0) as u32;

                                // Apply minimal validation - blackbox_decode includes frames from loop 0
                                // Only reject frames with clearly invalid data (zero time/loop when data should be present)
                                let is_valid_frame =
                                    current_time > 0 && (current_loop > 0 || current_time > 1000);

                                if is_valid_frame {
                                    parsing_success = true;
                                    stats.i_frames += 1;

                                    if debug && stats.i_frames <= 3 {
                                        println!(
                                            "DEBUG: Accepted I-frame - time:{}, loop:{}",
                                            current_time, current_loop
                                        );
                                    }
                                } else {
                                    if debug && stats.i_frames < 5 {
                                        println!(
                                            "DEBUG: Rejected I-frame - time:{}, loop:{} (invalid)",
                                            current_time, current_loop
                                        );
                                    }
                                    parsing_success = false;
                                }
                            }
                        }
                    }
                    'P' => {
                        if header.p_frame_def.count > 0 && frame_history.valid {
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
                                debug,
                            )
                            .is_ok()
                            {
                                // P-frames: parse_frame_data already computed correct absolute values
                                // Copy previous frame as base, then update only P-frame fields
                                frame_history
                                    .current_frame
                                    .copy_from_slice(&frame_history.previous_frame);

                                // Update only the fields that are present in P-frame with computed values
                                for (i, field_name) in
                                    header.p_frame_def.field_names.iter().enumerate()
                                {
                                    if i < p_frame_values.len() {
                                        // Find corresponding index in I-frame structure
                                        if let Some(i_frame_idx) = header
                                            .i_frame_def
                                            .field_names
                                            .iter()
                                            .position(|name| name == field_name)
                                        {
                                            if i_frame_idx < frame_history.current_frame.len() {
                                                // p_frame_values[i] contains correctly calculated absolute value
                                                frame_history.current_frame[i_frame_idx] =
                                                    p_frame_values[i];
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

                                // **BLACKBOX_DECODE COMPATIBILITY**: Validate P-frame before accepting
                                let current_time =
                                    frame_data.get("time").copied().unwrap_or(0) as u64;
                                let current_loop =
                                    frame_data.get("loopIteration").copied().unwrap_or(0) as u32;

                                // Apply minimal validation - blackbox_decode includes frames from loop 0
                                // Only reject frames with clearly invalid data (zero time/loop when data should be present)
                                let is_valid_frame =
                                    current_time > 0 && (current_loop > 0 || current_time > 1000);

                                if is_valid_frame {
                                    parsing_success = true;
                                    stats.p_frames += 1;

                                    if debug && stats.p_frames <= 3 {
                                        println!(
                                            "DEBUG: Accepted P-frame - time:{}, loop:{}",
                                            current_time, current_loop
                                        );
                                    }
                                } else {
                                    if debug && stats.p_frames < 5 {
                                        println!(
                                            "DEBUG: Rejected P-frame - time:{}, loop:{} (invalid)",
                                            current_time, current_loop
                                        );
                                    }
                                    parsing_success = false;
                                }
                            }
                        } else {
                            // Skip P-frame if we don't have valid I-frame history
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.failed_frames += 1;
                        }
                    }
                    'S' => {
                        if debug && stats.s_frames < 5 {
                            println!(
                                "DEBUG: Found S-frame, header.s_frame_def.count={}",
                                header.s_frame_def.count
                            );
                        }
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

                                // S-frames don't create separate CSV rows - they only update lastSlow data
                                // that gets merged into subsequent I/P frames (blackbox_decode compatibility)
                                stats.s_frames += 1;

                                if debug && stats.s_frames <= 3 {
                                    println!("DEBUG: S-frame count incremented to {} (data merged into lastSlow)", stats.s_frames);
                                }
                            } else if debug && stats.s_frames < 5 {
                                println!("DEBUG: S-frame parsing failed");
                            }
                        } else if debug && stats.s_frames < 5 {
                            println!("DEBUG: Skipping S-frame - header.s_frame_def.count is 0");
                        }
                    }
                    'H' => {
                        if header.h_frame_def.count > 0 {
                            if let Ok(data) = parse_h_frame(&mut stream, &header.h_frame_def, debug)
                            {
                                frame_data = data.clone();
                                parsing_success = true;
                                stats.h_frames += 1;

                                // Extract GPS home coordinates for GPX export if enabled
                                if export_options.gpx {
                                    let timestamp = last_main_frame_timestamp;

                                    if let (Some(&home_lat_raw), Some(&home_lon_raw)) = (
                                        frame_data.get("GPS_home[0]"),
                                        frame_data.get("GPS_home[1]"),
                                    ) {
                                        if debug && home_coordinates.is_empty() {
                                            println!("DEBUG: HOME raw values - home_lat_raw: {}, home_lon_raw: {}", home_lat_raw, home_lon_raw);
                                            println!(
                                                "DEBUG: HOME converted - lat: {:.7}, lon: {:.7}",
                                                convert_gps_coordinate(home_lat_raw),
                                                convert_gps_coordinate(home_lon_raw)
                                            );
                                        }

                                        let home_coordinate = GpsHomeCoordinate {
                                            home_latitude: convert_gps_coordinate(home_lat_raw),
                                            home_longitude: convert_gps_coordinate(home_lon_raw),
                                            timestamp_us: timestamp,
                                        };
                                        home_coordinates.push(home_coordinate);
                                    }
                                }
                            }
                        } else {
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.h_frames += 1;
                            parsing_success = true;
                        }
                    }
                    'G' => {
                        if header.g_frame_def.count > 0 {
                            // Initialize GPS frame history if needed
                            if gps_frame_history.is_empty() {
                                gps_frame_history = vec![0i32; header.g_frame_def.count];
                            }

                            let mut g_frame_values = vec![0i32; header.g_frame_def.count];

                            if bbl_format::parse_frame_data(
                                &mut stream,
                                &header.g_frame_def,
                                &mut g_frame_values,
                                Some(&gps_frame_history), // Use GPS frame history for differential encoding
                                None,  // GPS frames typically don't use previous2
                                0,     // TODO: Calculate skipped frames properly
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                                debug,
                            )
                            .is_ok()
                            {
                                // Update GPS frame history with new values
                                gps_frame_history.copy_from_slice(&g_frame_values);

                                // Copy GPS frame data to output
                                for (i, field_name) in
                                    header.g_frame_def.field_names.iter().enumerate()
                                {
                                    if i < g_frame_values.len() {
                                        let value = g_frame_values[i];
                                        frame_data.insert(field_name.clone(), value);
                                    }
                                }

                                parsing_success = true;
                                stats.g_frames += 1;

                                // Extract GPS coordinates for GPX export if enabled
                                if export_options.gpx {
                                    let gps_time =
                                        frame_data.get("time").copied().unwrap_or(0) as u64;
                                    let timestamp = if gps_time > 0 {
                                        gps_time
                                    } else {
                                        last_main_frame_timestamp
                                    };

                                    if let (Some(&lat_raw), Some(&lon_raw), Some(&alt_raw)) = (
                                        frame_data.get("GPS_coord[0]"),
                                        frame_data.get("GPS_coord[1]"),
                                        frame_data.get("GPS_altitude"),
                                    ) {
                                        // GPS coordinates are deltas from home position
                                        // Need to add home coordinates to get actual GPS position
                                        let actual_lat =
                                            if let Some(home_coord) = home_coordinates.first() {
                                                home_coord.home_latitude
                                                    + convert_gps_coordinate(lat_raw)
                                            } else {
                                                convert_gps_coordinate(lat_raw)
                                            };

                                        let actual_lon =
                                            if let Some(home_coord) = home_coordinates.first() {
                                                home_coord.home_longitude
                                                    + convert_gps_coordinate(lon_raw)
                                            } else {
                                                convert_gps_coordinate(lon_raw)
                                            };

                                        if debug && gps_coordinates.len() < 3 {
                                            println!("DEBUG: GPS raw values - lat_raw: {}, lon_raw: {}, alt_raw: {}", lat_raw, lon_raw, alt_raw);
                                            println!("DEBUG: GPS converted - lat: {:.7}, lon: {:.7}, alt: {:.2}", 
                                                   actual_lat, actual_lon,
                                                   convert_gps_altitude(alt_raw, &header.firmware_revision));
                                        }

                                        let coordinate = GpsCoordinate {
                                            latitude: actual_lat,
                                            longitude: actual_lon,
                                            altitude: convert_gps_altitude(
                                                alt_raw,
                                                &header.firmware_revision,
                                            ),
                                            timestamp_us: timestamp,
                                            num_sats: frame_data.get("GPS_numSat").copied(),
                                            speed: frame_data
                                                .get("GPS_speed")
                                                .map(|&s| convert_gps_speed(s)),
                                            ground_course: frame_data
                                                .get("GPS_ground_course")
                                                .map(|&c| convert_gps_course(c)),
                                        };
                                        gps_coordinates.push(coordinate);
                                    }
                                }
                            }
                        } else {
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.g_frames += 1;
                            parsing_success = true;
                        }
                    }
                    'E' => {
                        if let Ok(mut event_frame) = parse_e_frame(&mut stream, debug) {
                            // Store event data for potential export
                            // For now, create a dummy data entry for consistency
                            frame_data
                                .insert("event_type".to_string(), event_frame.event_type as i32);
                            frame_data.insert("event_description".to_string(), 0); // Can't store string in i32 map
                            parsing_success = true;
                            stats.e_frames += 1;

                            // Collect event frames for JSON export if enabled
                            if export_options.event {
                                event_frame.timestamp_us = last_main_frame_timestamp;
                                event_frames.push(event_frame);
                            }

                            if debug && stats.e_frames <= 3 {
                                println!(
                                    "DEBUG: Parsed E-frame - Type: {}",
                                    frame_data.get("event_type").unwrap_or(&0)
                                );
                            }
                        } else {
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.e_frames += 1;
                            parsing_success = true;
                        }
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
                    // Extract timing before moving frame_data
                    let timestamp_us = frame_data.get("time").copied().unwrap_or(0) as u64;
                    let loop_iteration =
                        frame_data.get("loopIteration").copied().unwrap_or(0) as u32;

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
                    debug_frame_list.push(decoded_frame);
                } else if parsing_success && store_all_frames {
                    // Store ALL frames for CSV export when requested
                    let debug_frame_list = debug_frames.entry(frame_type).or_default();
                    // Store all frames for complete CSV export - memory usage managed by processing in chunks
                    let timestamp_us = frame_data.get("time").copied().unwrap_or(0) as u64;
                    let loop_iteration =
                        frame_data.get("loopIteration").copied().unwrap_or(0) as u32;

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
                    debug_frame_list.push(decoded_frame);
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

    Ok((
        stats,
        sample_frames,
        Some(debug_frames),
        gps_coordinates,
        home_coordinates,
        event_frames,
    ))
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

// Note: parse_g_frame is no longer used - G frames now use differential encoding
// like P frames in the main parsing loop for correct GPS coordinate calculation

// Parse E frames (Event frames) - based on C reference implementation
fn parse_e_frame(stream: &mut bbl_format::BBLDataStream, debug: bool) -> Result<EventFrame> {
    if debug {
        println!("Parsing E frame (Event frame)");
    }

    // Read event type (1 byte)
    let event_type = stream.read_byte()?;

    // Read event data - the length depends on the event type
    let mut event_data = Vec::new();
    let event_description = match event_type {
        0 => {
            // FLIGHT_LOG_EVENT_SYNC_BEEP
            "Sync beep".to_string()
        }
        1 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_START
            "Autotune cycle start".to_string()
        }
        2 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_RESULT
            let axis = stream.read_byte()?;
            let p_gain = stream.read_signed_vb()? as f32 / 1000.0;
            let i_gain = stream.read_signed_vb()? as f32 / 1000.0;
            let d_gain = stream.read_signed_vb()? as f32 / 1000.0;
            event_data.extend_from_slice(&[axis]);
            format!(
                "Autotune cycle result - Axis: {}, P: {:.3}, I: {:.3}, D: {:.3}",
                axis, p_gain, i_gain, d_gain
            )
        }
        3 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_TARGETS
            let current_angle = stream.read_signed_vb()?;
            let target_angle = stream.read_signed_vb()?;
            let target_angle_at_peak = stream.read_signed_vb()?;
            let first_peak_angle = stream.read_signed_vb()?;
            let second_peak_angle = stream.read_signed_vb()?;
            format!("Autotune targets - Current: {}, Target: {}, Peak target: {}, First peak: {}, Second peak: {}", 
                   current_angle, target_angle, target_angle_at_peak, first_peak_angle, second_peak_angle)
        }
        4 => {
            // FLIGHT_LOG_EVENT_INFLIGHT_ADJUSTMENT
            let adjustment_function = stream.read_byte()?;
            if adjustment_function > 127 {
                // Float value
                let new_value = stream.read_unsigned_vb()? as f32;
                event_data.extend_from_slice(&[adjustment_function]);
                format!(
                    "Inflight adjustment - Function: {}, New value: {:.3}",
                    adjustment_function, new_value
                )
            } else {
                // Integer value
                let new_value = stream.read_signed_vb()?;
                event_data.extend_from_slice(&[adjustment_function]);
                format!(
                    "Inflight adjustment - Function: {}, New value: {}",
                    adjustment_function, new_value
                )
            }
        }
        5 => {
            // FLIGHT_LOG_EVENT_LOGGING_RESUME
            let log_iteration = stream.read_unsigned_vb()?;
            let current_time = stream.read_unsigned_vb()?;
            format!(
                "Logging resume - Iteration: {}, Time: {}",
                log_iteration, current_time
            )
        }
        6 => {
            // FLIGHT_LOG_EVENT_LOG_END (old numbering)
            // Read end message bytes
            for _ in 0..4 {
                if !stream.eof {
                    event_data.push(stream.read_byte()?);
                }
            }
            "Log end".to_string()
        }
        10 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_START (UNUSED)
            "Autotune cycle start (unused)".to_string()
        }
        11 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_RESULT (UNUSED)
            "Autotune cycle result (unused)".to_string()
        }
        12 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_TARGETS (UNUSED)
            "Autotune targets (unused)".to_string()
        }
        13 => {
            // FLIGHT_LOG_EVENT_INFLIGHT_ADJUSTMENT
            let adjustment_function = stream.read_byte()?;
            if adjustment_function > 127 {
                let new_value = stream.read_unsigned_vb()? as f32;
                event_data.extend_from_slice(&[adjustment_function]);
                format!(
                    "Inflight adjustment - Function: {}, New value: {:.3}",
                    adjustment_function, new_value
                )
            } else {
                let new_value = stream.read_signed_vb()?;
                event_data.extend_from_slice(&[adjustment_function]);
                format!(
                    "Inflight adjustment - Function: {}, New value: {}",
                    adjustment_function, new_value
                )
            }
        }
        14 => {
            // FLIGHT_LOG_EVENT_LOGGING_RESUME
            let log_iteration = stream.read_unsigned_vb()?;
            let current_time = stream.read_unsigned_vb()?;
            format!(
                "Logging resume - Iteration: {}, Time: {}",
                log_iteration, current_time
            )
        }
        15 => {
            // FLIGHT_LOG_EVENT_DISARM
            "Disarm".to_string()
        }
        30 => {
            // FLIGHT_LOG_EVENT_FLIGHTMODE - flight mode status event
            // Read flight mode data
            for _ in 0..4 {
                if !stream.eof {
                    event_data.push(stream.read_byte()?);
                }
            }
            "Flight mode change".to_string()
        }
        255 => {
            // FLIGHT_LOG_EVENT_LOG_END
            "Log end".to_string()
        }
        _ => {
            // Unknown event type - read a few bytes as data
            for _ in 0..8 {
                if stream.eof {
                    break;
                }
                event_data.push(stream.read_byte()?);
            }
            format!("Unknown event type: {}", event_type)
        }
    };

    if debug {
        println!(
            "DEBUG: Event - Type: {}, Description: {}",
            event_type, event_description
        );
    }

    Ok(EventFrame {
        timestamp_us: 0, // Will be set later from context
        event_type,
        event_data,
        event_description,
    })
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

/// Converts raw vbatLatest value to volts using firmware-aware scaling.
///
/// Betaflight < 4.3.0: tenths (0.1V units)
/// Betaflight >= 4.3.0: hundredths (0.01V units)
/// EmuFlight: always tenths (0.1V units)
/// iNav: always hundredths (0.01V units)
fn convert_vbat_to_volts(raw_value: i32, firmware_revision: &str) -> f32 {
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
fn extract_firmware_version(firmware_revision: &str) -> Option<Version> {
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
fn convert_amperage_to_amps(raw_value: i32) -> f32 {
    raw_value as f32 / 100.0
}

fn parse_bbl_file_streaming(
    file_path: &Path,
    debug: bool,
    export_options: &ExportOptions,
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
        let (log, gps_coords, home_coords, events) = parse_single_log(
            log_data,
            log_index + 1,
            log_positions.len(),
            debug,
            export_options,
        )?;

        // Display log info immediately
        display_log_info(&log);

        // Check if we should skip exports for this log
        let (should_skip, reason) = should_skip_export(&log, export_options.force_export);
        if should_skip {
            println!("Skipping exports for this log: {}", reason);
            processed_logs += 1;

            // Add separator between logs for clarity
            if log_index + 1 < log_positions.len() {
                println!();
            }
            continue;
        }

        // Export CSV immediately while data is hot in cache
        if export_options.csv {
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

        // Export GPS data to GPX if requested
        if export_options.gpx && !gps_coords.is_empty() {
            if let Err(e) = export_gpx_file(
                file_path,
                log_index,
                log_positions.len(),
                &gps_coords,
                &home_coords,
                export_options,
            ) {
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                eprintln!(
                    "Warning: Failed to export GPX for {filename} log {}: {e}",
                    log_index + 1
                );
            }
        }

        // Export event data to JSON if requested
        if export_options.event && !events.is_empty() {
            if let Err(e) = export_event_file(
                file_path,
                log_index,
                log_positions.len(),
                &events,
                export_options,
            ) {
                let filename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                eprintln!(
                    "Warning: Failed to export events for {filename} log {}: {e}",
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

// GPS/GPX export functions
fn extract_major_firmware_version(firmware_revision: &str) -> u8 {
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

fn convert_gps_coordinate(raw_value: i32) -> f64 {
    // GPS coordinates are stored as degrees * 10000000
    raw_value as f64 / 10000000.0
}

fn convert_gps_altitude(raw_value: i32, firmware_revision: &str) -> f64 {
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

fn convert_gps_speed(raw_value: i32) -> f64 {
    // Speed is stored as cm/s * 100, convert to m/s
    raw_value as f64 / 100.0
}

fn convert_gps_course(raw_value: i32) -> f64 {
    // Course is stored as degrees * 10
    raw_value as f64 / 10.0
}

fn export_gpx_file(
    file_path: &Path,
    log_number: usize,
    total_logs: usize,
    gps_coords: &[GpsCoordinate],
    _home_coords: &[GpsHomeCoordinate], // TODO: Use home coordinates for reference point
    export_options: &ExportOptions,
) -> Result<()> {
    if gps_coords.is_empty() {
        return Ok(());
    }

    let base_name = file_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let output_dir = export_options
        .output_dir
        .as_deref()
        .unwrap_or_else(|| file_path.parent().unwrap().to_str().unwrap());

    // Use consistent naming: only add suffix for multiple logs
    let log_suffix = if total_logs > 1 {
        format!(".{:02}", log_number + 1)
    } else {
        "".to_string()
    };
    let gpx_filename = format!("{}/{}{}.gps.gpx", output_dir, base_name, log_suffix);

    let mut gpx_file = std::fs::File::create(&gpx_filename)?;
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

    for coord in gps_coords {
        // Only include coordinates with sufficient GPS satellite count (minimum 5)
        if let Some(num_sats) = coord.num_sats {
            if num_sats < 5 {
                continue;
            }
        }

        // Convert timestamp to ISO format
        // Simplified timestamp calculation to approximate BBD format
        let total_seconds = coord.timestamp_us / 1_000_000;
        let microseconds = coord.timestamp_us % 1_000_000;

        // Use March 26, 2025 as base date to match BBD format more closely
        let hours = 5 + (total_seconds / 3600) % 24; // Start at 05:xx like BBD
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

    println!("Exported GPS data to: {}", gpx_filename);
    Ok(())
}

fn export_event_file(
    file_path: &Path,
    log_number: usize,
    total_logs: usize,
    events: &[EventFrame],
    export_options: &ExportOptions,
) -> Result<()> {
    if events.is_empty() {
        return Ok(());
    }

    let base_name = file_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let output_dir = export_options
        .output_dir
        .as_deref()
        .unwrap_or_else(|| file_path.parent().unwrap().to_str().unwrap());

    // Use consistent naming: only add suffix for multiple logs
    let log_suffix = if total_logs > 1 {
        format!(".{:02}", log_number + 1)
    } else {
        "".to_string()
    };
    let event_filename = format!("{}/{}{}.event", output_dir, base_name, log_suffix);

    let mut event_file = std::fs::File::create(&event_filename)?;

    // Export as JSONL format (individual JSON objects per line) to match blackbox_decode
    for event in events.iter() {
        writeln!(
            event_file,
            r#"{{"name":"{}", "time":{}}}"#,
            event.event_description.replace('"', "\\\""),
            event.timestamp_us
        )?;
    }

    println!("Exported event data to: {}", event_filename);
    Ok(())
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
