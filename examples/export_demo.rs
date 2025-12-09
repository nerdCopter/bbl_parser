//! Example demonstrating BBL export functionality
//!
//! This example shows how to use the bbl_parser crate to parse BBL files
//! and export data to CSV, GPX, and Event formats programmatically.

use anyhow::Result;
use bbl_parser::{export_to_csv, export_to_event, export_to_gpx, parse_bbl_file, ExportOptions};
use std::path::Path;

fn main() -> Result<()> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <bbl_file> [output_dir]", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} flight.BBL", args[0]);
        eprintln!("  {} flight.BBL ./output", args[0]);
        std::process::exit(1);
    }

    let input_file = &args[1];
    let output_dir = args.get(2).map(|s| s.to_string());

    println!("=== BBL Parser Export Demo ===");
    println!("Input file: {}", input_file);
    if let Some(ref dir) = output_dir {
        println!("Output directory: {}", dir);
    }
    println!();

    // Configure export options
    let export_opts = ExportOptions {
        csv: true,
        gpx: true,
        event: true,
        output_dir: output_dir.clone(),
        force_export: false,
    };

    // Parse the BBL file
    println!("Parsing BBL file...");
    let input_path = Path::new(input_file);
    let log = parse_bbl_file(input_path, export_opts.clone(), false)?;

    // Display basic information
    println!("\n=== Log Information ===");
    println!("Firmware: {}", log.header.firmware_revision);
    if !log.header.board_info.is_empty() {
        println!("Board: {}", log.header.board_info);
    }
    if !log.header.craft_name.is_empty() {
        println!("Craft: {}", log.header.craft_name);
    }
    println!("Data version: {}", log.header.data_version);
    println!("Looptime: {} μs", log.header.looptime);
    println!();

    println!("=== Frame Statistics ===");
    println!("Total frames: {}", log.stats.total_frames);
    println!("I frames: {}", log.stats.i_frames);
    println!("P frames: {}", log.stats.p_frames);
    if log.stats.s_frames > 0 {
        println!("S frames: {}", log.stats.s_frames);
    }
    if log.stats.g_frames > 0 {
        println!("G frames: {}", log.stats.g_frames);
    }
    if log.stats.h_frames > 0 {
        println!("H frames: {}", log.stats.h_frames);
    }
    if log.stats.e_frames > 0 {
        println!("E frames: {}", log.stats.e_frames);
    }
    println!();

    // Display timing information
    if log.stats.start_time_us > 0 && log.stats.end_time_us > log.stats.start_time_us {
        let duration_us = log.stats.end_time_us - log.stats.start_time_us;
        let duration_s = duration_us as f64 / 1_000_000.0;
        println!("Duration: {:.2}s ({} μs)", duration_s, duration_us);
        println!();
    }

    // Export CSV
    println!("=== Exporting Data ===");
    println!("Exporting CSV files...");
    export_to_csv(&log, input_path, &export_opts)?;
    println!("✓ CSV export complete");

    // Compute log index once (log_number is 1-based)
    let log_index = log.log_number.checked_sub(1).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid log number: {} cannot be used to compute index",
            log.log_number
        )
    })?;

    // Export GPX if GPS data exists
    if !log.gps_coordinates.is_empty() {
        println!(
            "Exporting GPX file ({} GPS coordinates)...",
            log.gps_coordinates.len()
        );
        export_to_gpx(
            input_path,
            log_index,
            log.total_logs,
            &log.gps_coordinates,
            &log.home_coordinates,
            &export_opts,
            log.header.log_start_datetime.as_deref(),
        )?;
        println!("✓ GPX export complete");
    } else {
        println!("⊘ No GPS data to export");
    }

    // Export events if event data exists
    if !log.event_frames.is_empty() {
        println!(
            "Exporting event file ({} events)...",
            log.event_frames.len()
        );
        export_to_event(
            input_path,
            log_index,
            log.total_logs,
            &log.event_frames,
            &export_opts,
        )?;
        println!("✓ Event export complete");

        // Display sample events
        println!("\n=== Sample Events ===");
        for (i, event) in log.event_frames.iter().take(5).enumerate() {
            println!(
                "  {}. {} (time: {} μs)",
                i + 1,
                event.event_name,
                event.timestamp_us
            );
        }
        if log.event_frames.len() > 5 {
            println!("  ... and {} more events", log.event_frames.len() - 5);
        }
    } else {
        println!("⊘ No event data to export");
    }

    println!("\n=== Export Complete ===");
    println!("All requested exports completed successfully!");

    Ok(())
}
