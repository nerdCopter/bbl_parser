//! Multi-Flight CSV Export Example
//!
//! Demonstrates how to export ALL flights/logs from a BBL file to CSV.
//! This is important because a single BBL file can contain multiple flight sessions.
//!
//! Key difference from csv_export.rs:
//! - csv_export.rs: Uses parse_bbl_file() - exports FIRST log only
//! - This example: Uses parse_bbl_file_all_logs() - exports ALL logs with proper suffixes

use bbl_parser::{export_to_csv, parse_bbl_file_all_logs, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Get input file from command line or show usage
    let input_file = std::env::args().nth(1).unwrap_or_else(|| {
        println!("Usage: multi_flight_export <input.BBL> [output_dir]");
        println!("Example: multi_flight_export flight.BBL ./output");
        println!("\nThis example exports ALL flights from a BBL file.");
        println!("Files will be named with suffixes: .01.csv, .02.csv, .03.csv, etc.");
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

    // Parse ALL logs from the BBL file (not just the first)
    println!("Parsing: {}", input_file);
    let logs = parse_bbl_file_all_logs(Path::new(&input_file), export_opts.clone(), false)?;

    println!("✓ Found {} flight log(s)\n", logs.len());

    if logs.is_empty() {
        println!("No logs found in file.");
        return Ok(());
    }

    // Export each flight with proper numbering
    for log in logs {
        println!("Flight {}/{}:", log.log_number, log.total_logs);
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
        println!("  Exporting to CSV...");
        export_to_csv(&log, Path::new(&input_file), &export_opts)?;

        // Show output filename with flight number suffix
        let suffix = if log.total_logs > 1 {
            format!(".{:02}", log.log_number)
        } else {
            String::new()
        };
        println!(
            "  ✓ Exported{}\n",
            if suffix.is_empty() {
                "".to_string()
            } else {
                format!(" as .{:02}.csv", log.log_number)
            }
        );
    }

    println!("✓ All CSV exports complete");
    Ok(())
}
