use anyhow::Result;
use clap::{Arg, Command};
use glob::glob;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

// Import export functions from crate library
use bbl_parser::export::{export_to_csv, export_to_event, export_to_gpx};

// Import parser types from crate library - using crate's unified implementations
use bbl_parser::parser::{parse_frames, parse_headers_from_text};

// Import types from crate library
use bbl_parser::types::BBLLog;

// Test-only imports
#[cfg(test)]
use bbl_parser::conversion::{
    convert_amperage_to_amps, convert_vbat_to_volts, format_failsafe_phase,
    format_flight_mode_flags, format_state_flags,
};
#[cfg(test)]
use bbl_parser::types::{BBLHeader, DecodedFrame, FrameDefinition, FrameStats};

// Import ExportOptions from crate library
use bbl_parser::ExportOptions;

// Include vergen generated environment variables
const GIT_SHA: &str = env!("VERGEN_GIT_SHA", "unknown");
const GIT_COMMIT_DATE: &str = env!("VERGEN_GIT_COMMIT_DATE", "unknown");

// Build version string from git info
const VERSION_STR: &str = concat!(
    env!("VERGEN_GIT_SHA", "unknown"),
    " (",
    env!("VERGEN_GIT_COMMIT_DATE", "unknown"),
    ")"
);

/// Maximum recursion depth to prevent stack overflow
const MAX_RECURSION_DEPTH: usize = 100;

/// Get output directory from export options, falling back to file's parent directory or ".".
fn get_output_dir<'a>(export_options: &'a ExportOptions, file_path: &'a Path) -> &'a str {
    export_options
        .output_dir
        .as_deref()
        .unwrap_or_else(|| file_path.parent().and_then(|p| p.to_str()).unwrap_or("."))
}

/// Helper to compute export file paths and suffixes for status messages.
/// Computes base filename, output directory, and log suffix (with .NN suffix only for multiple logs).
/// Uses log_number (1-based) directly to match export.rs behavior.
/// Returns (csv_path, headers_path, gpx_path, event_path) for consistency across platforms.
fn format_export_path(
    file_path: &Path,
    export_options: &ExportOptions,
    log_number: usize,
    total_logs: usize,
) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let base_name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("blackbox");

    let output_dir = Path::new(get_output_dir(export_options, file_path));
    let log_suffix = if total_logs > 1 {
        format!(".{:02}", log_number)
    } else {
        String::new()
    };

    let csv_filename = format!("{}{}.csv", base_name, log_suffix);
    let headers_filename = format!("{}{}.headers.csv", base_name, log_suffix);
    let gpx_filename = format!("{}{}.gps.gpx", base_name, log_suffix);
    let event_filename = format!("{}{}.event", base_name, log_suffix);

    (
        output_dir.join(&csv_filename),
        output_dir.join(&headers_filename),
        output_dir.join(&gpx_filename),
        output_dir.join(&event_filename),
    )
}

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
                        Ok(mut paths) => {
                            paths.sort(); // deterministic ordering
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
                    // It's a file; dedupe using visited
                    if visited.insert(canonical_path.clone()) {
                        if let Some(path_str) = canonical_path.to_str() {
                            bbl_files.push(path_str.to_string());
                        }
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

#[allow(dead_code)]
fn should_have_frame(frame_index: u32, sysconfig: &HashMap<String, i32>) -> bool {
    let frame_interval_i = sysconfig.get("frameIntervalI").copied().unwrap_or(32);
    let frame_interval_p_num = sysconfig.get("frameIntervalPNum").copied().unwrap_or(1);
    let frame_interval_p_denom = sysconfig.get("frameIntervalPDenom").copied().unwrap_or(1);

    let left_side = ((frame_index % frame_interval_i as u32) + frame_interval_p_num as u32 - 1)
        % frame_interval_p_denom as u32;
    left_side < frame_interval_p_num as u32
}

fn build_command() -> Command {
    let about_text = format!(
        "\n\nRead and parse BBL blackbox log files. Exports to CSV by default (optionally GPX/JSON).\n  {} {} ({})",
        env!("CARGO_PKG_NAME"), GIT_SHA, GIT_COMMIT_DATE
    );

    Command::new(env!("CARGO_PKG_NAME"))
        .version(VERSION_STR)
        .about(about_text)
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

    let mut processed_files = 0;

    if debug {
        println!("Input patterns: {file_patterns:?}");
    }

    // Expand input paths (files and directories) to a list of BBL files
    let mut visited = HashSet::new();
    let mut input_files = match expand_input_paths(
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

    // Dedupe while preserving original order
    {
        let mut seen = std::collections::HashSet::new();
        input_files.retain(|p| seen.insert(p.clone()));
    }

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

        match parse_bbl_file_streaming(path, debug, &export_options) {
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
        let log = parse_single_log(
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

/// Parse a single log from binary data.
///
/// Parses all frames and stores them in BBLLog.frames for CSV export.
fn parse_single_log(
    log_data: &[u8],
    log_number: usize,
    total_logs: usize,
    debug: bool,
    export_options: &ExportOptions,
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
            "DEBUG: Total frames: {}, Stored frames: {}",
            stats.total_frames,
            frames.len()
        );
    }

    let log = BBLLog {
        log_number,
        total_logs,
        header,
        stats,
        frames,
        debug_frames,
        gps_coordinates: gps_coords,
        home_coordinates: home_coords,
        event_frames: events,
    };

    Ok(log)
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

    // Fallback to frames if debug_frames not available or insufficient data
    if gyro_x_values.len() < MIN_SAMPLES_FOR_ANALYSIS {
        for frame in &log.frames {
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

fn parse_bbl_file_streaming(
    file_path: &Path,
    debug: bool,
    export_options: &ExportOptions,
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
            match export_to_csv(&log, file_path, export_options) {
                Ok(()) => {
                    let (csv_path, headers_path, _, _) = format_export_path(
                        file_path,
                        export_options,
                        log.log_number,
                        log_positions.len(),
                    );
                    println!("Exported headers to: {}", headers_path.display());
                    println!("Exported flight data to: {}", csv_path.display());
                }
                Err(e) => {
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
        }

        // Export GPS data to GPX if requested
        if export_options.gpx && !log.gps_coordinates.is_empty() {
            match export_to_gpx(
                file_path,
                log_index,
                log_positions.len(),
                &log.gps_coordinates,
                &log.home_coordinates,
                export_options,
                log.header.log_start_datetime.as_deref(),
            ) {
                Ok(()) => {
                    let (_, _, gpx_path, _) = format_export_path(
                        file_path,
                        export_options,
                        log.log_number,
                        log_positions.len(),
                    );
                    println!("Exported GPS data to: {}", gpx_path.display());
                }
                Err(e) => {
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
        }

        // Export event data to JSON if requested
        if export_options.event && !log.event_frames.is_empty() {
            match export_to_event(
                file_path,
                log_index,
                log_positions.len(),
                &log.event_frames,
                export_options,
            ) {
                Ok(()) => {
                    let (_, _, _, event_path) = format_export_path(
                        file_path,
                        export_options,
                        log.log_number,
                        log_positions.len(),
                    );
                    println!("Exported event data to: {}", event_path.display());
                }
                Err(e) => {
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
    fn test_export_options() {
        let options = ExportOptions {
            csv: true,
            gpx: false,
            event: false,
            output_dir: Some("/tmp".to_string()),
            force_export: false,
        };
        assert_eq!(options.output_dir.as_ref().unwrap(), "/tmp");
        assert!(options.csv);
        assert!(!options.gpx);
        assert!(!options.event);
        assert!(!options.force_export);

        // Test default configuration (all false except output_dir which is None)
        let options = ExportOptions::default();
        assert!(options.output_dir.is_none());
        assert!(!options.csv);
        assert!(!options.gpx);
        assert!(!options.event);
        assert!(!options.force_export);
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
            log_start_datetime: None,
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
