//! CLI binary for BBL Parser
//!
//! This provides the command-line interface for the BBL parser library.

use anyhow::Result;
use bbl_parser::{parse_bbl_file, ExportOptions};
use clap::{Arg, Command};
use glob::glob;
use std::path::Path;

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
        .get_matches();

    let debug = matches.get_flag("debug");
    let export_csv = matches.get_flag("csv");
    let export_gpx = matches.get_flag("gpx") || matches.get_flag("gps");
    let export_event = matches.get_flag("event");
    let output_dir = matches.get_one::<String>("output-dir").cloned();
    let file_patterns: Vec<&String> = matches.get_many::<String>("files").unwrap().collect();

    let export_options = ExportOptions {
        csv: export_csv,
        gpx: export_gpx,
        event: export_event,
        output_dir: output_dir.clone(),
    };

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

    // Process files using the library API
    for (index, path) in valid_paths.iter().enumerate() {
        if index > 0 {
            println!();
        }

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        println!("Processing: {filename}");

        // For now, use a placeholder - we'll need to implement the actual parsing
        match parse_bbl_file(path, export_options.clone(), debug) {
            Ok(log) => {
                if debug {
                    println!(
                        "Successfully processed log with {} sample frames",
                        log.sample_frames.len()
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
