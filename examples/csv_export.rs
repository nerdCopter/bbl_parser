//! CSV Export Example
//!
//! Demonstrates how to export the FIRST flight/log from a BBL file to CSV format.
//! This is the primary export format compatible with blackbox_decode.
//!
//! **Important:** BBL files can contain multiple flights/logs (separated by LOG_END events).
//! This example exports only the first one using `parse_bbl_file()`.
//!
//! For multi-flight files, use `parse_bbl_file_all_logs()` instead.
//! See `multi_flight_export.rs` example for handling multiple flights.

use bbl_parser::{export_to_csv, parse_bbl_file, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Get input file from command line or show usage
    let input_file = std::env::args().nth(1).unwrap_or_else(|| {
        println!("Usage: csv_export <input.BBL> [output_dir]");
        println!("Example: csv_export flight.BBL ./output");
        println!("\nNote: This exports only the FIRST flight from the BBL file.");
        println!("For files with multiple flights, see multi_flight_export example.");
        std::process::exit(1);
    });

    // Get optional output directory from command line
    let output_dir = std::env::args().nth(2).map(|s| s.to_string());

    // Configure export options - CSV only
    let export_opts = ExportOptions {
        csv: true,
        gpx: false,
        event: false,
        output_dir: output_dir.clone(),
        force_export: false,
    };

    // Parse the BBL file
    println!("Parsing: {}", input_file);
    let log = parse_bbl_file(Path::new(&input_file), export_opts.clone(), false)?;

    // Display log information
    println!("\nLog Information:");
    println!("  Firmware: {}", log.header.firmware_revision);
    println!("  Board: {}", log.header.board_info);
    if !log.header.craft_name.is_empty() {
        println!("  Craft: {}", log.header.craft_name);
    }
    println!("  Total frames: {}", log.stats.total_frames);

    if log.stats.start_time_us > 0 && log.stats.end_time_us > log.stats.start_time_us {
        let duration_s = (log.stats.end_time_us - log.stats.start_time_us) as f64 / 1_000_000.0;
        println!("  Duration: {:.2}s", duration_s);
    }

    // Export to CSV
    println!("\nExporting to CSV...");
    export_to_csv(&log, Path::new(&input_file), &export_opts)?;
    println!("✓ CSV export complete");

    Ok(())
}
