//! Multi-Format Export Example
//!
//! Demonstrates how to export all available formats (CSV, GPX, Event) in one program.
//! Shows conditional export based on data availability.

use bbl_parser::{export_to_csv, export_to_event, export_to_gpx, parse_bbl_file, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Get input file from command line or show usage
    let input_file = std::env::args().nth(1).unwrap_or_else(|| {
        println!("Usage: multi_export <input.BBL> [output_dir]");
        println!("Example: multi_export flight.BBL ./output");
        std::process::exit(1);
    });

    // Optional output directory
    let output_dir = std::env::args().nth(2).map(|s| s.to_string());

    // Configure export options - all formats enabled
    let export_opts = ExportOptions {
        csv: true,
        gpx: true,
        event: true,
        output_dir: output_dir.clone(),
        force_export: false,
    };

    // Parse the BBL file
    println!("Parsing: {}", input_file);
    let log = parse_bbl_file(Path::new(&input_file), export_opts.clone(), false)?;

    // Display comprehensive log information
    println!("\n=== Log Information ===");
    println!("Firmware: {}", log.header.firmware_revision);
    println!("Board: {}", log.header.board_info);
    if !log.header.craft_name.is_empty() {
        println!("Craft: {}", log.header.craft_name);
    }
    println!("Data version: {}", log.header.data_version);
    println!("Looptime: {} μs", log.header.looptime);

    println!("\n=== Frame Statistics ===");
    println!("Total frames: {}", log.stats.total_frames);
    println!("  I frames: {}", log.stats.i_frames);
    println!("  P frames: {}", log.stats.p_frames);
    if log.stats.s_frames > 0 {
        println!("  S frames: {}", log.stats.s_frames);
    }
    if log.stats.g_frames > 0 {
        println!("  G frames (GPS): {}", log.stats.g_frames);
    }
    if log.stats.h_frames > 0 {
        println!("  H frames (Home): {}", log.stats.h_frames);
    }
    if log.stats.e_frames > 0 {
        println!("  E frames (Events): {}", log.stats.e_frames);
    }

    if log.stats.start_time_us > 0 && log.stats.end_time_us > log.stats.start_time_us {
        let duration_s = (log.stats.end_time_us - log.stats.start_time_us) as f64 / 1_000_000.0;
        println!("Duration: {:.2}s", duration_s);
    }

    // Export all available formats
    println!("\n=== Exporting Data ===");

    // CSV Export (always works)
    println!("Exporting CSV...");
    export_to_csv(&log, Path::new(&input_file), &export_opts)?;
    println!("✓ CSV export complete");

    // Compute log index once (log_number is 1-based)
    let log_index = log.log_number - 1;

    // GPS Export (if data available)
    if !log.gps_coordinates.is_empty() {
        println!("Exporting GPX...");
        export_to_gpx(
            Path::new(&input_file),
            log_index,
            log.total_logs,
            &log.gps_coordinates,
            &log.home_coordinates,
            &export_opts,
            log.header.log_start_datetime.as_deref(),
        )?;
        println!(
            "✓ GPX export complete ({} coordinates)",
            log.gps_coordinates.len()
        );
    } else if log.stats.g_frames > 0 {
        println!(
            "⊘ GPS frames present ({}) but not collected by parser",
            log.stats.g_frames
        );
    } else {
        println!("⊘ No GPS data available");
    }

    // Event Export (if data available)
    if !log.event_frames.is_empty() {
        println!("Exporting Events...");
        export_to_event(
            Path::new(&input_file),
            log_index,
            log.total_logs,
            &log.event_frames,
            &export_opts,
        )?;
        println!(
            "✓ Event export complete ({} events)",
            log.event_frames.len()
        );
    } else if log.stats.e_frames > 0 {
        println!(
            "⊘ Event frames present ({}) but not collected by parser",
            log.stats.e_frames
        );
    } else {
        println!("⊘ No event data available");
    }

    println!("\n=== Export Summary ===");
    if let Some(dir) = output_dir {
        println!("Output directory: {}", dir);
    }
    println!("✓ CSV files exported");
    println!(
        "{} GPS data exported",
        if log.gps_coordinates.is_empty() {
            "⊘"
        } else {
            "✓"
        }
    );
    println!(
        "{} Event data exported",
        if log.event_frames.is_empty() {
            "⊘"
        } else {
            "✓"
        }
    );

    Ok(())
}
