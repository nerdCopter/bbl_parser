//! GPX Export Example
//!
//! Demonstrates how to export GPS data to GPX format for use with mapping applications.
//! The parser module now fully supports GPS frame parsing (G-frames and H-frames).

use bbl_parser::{export_to_gpx, parse_bbl_file, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Get input file from command line or show usage
    let input_file = std::env::args().nth(1).unwrap_or_else(|| {
        println!("Usage: gpx_export <input.BBL> [output_dir]");
        println!("Example: gpx_export flight.BBL ./output");
        println!("Note: GPS export requires GPS data in the BBL file");
        std::process::exit(1);
    });

    // Get optional output directory from command line
    let output_dir = std::env::args().nth(2).map(|s| s.to_string());

    // Configure export options - GPX export enabled
    let export_opts = ExportOptions {
        csv: false,
        gpx: true,
        event: false,
        output_dir: output_dir.clone(),
        force_export: false,
    };

    // Parse the BBL file
    println!("Parsing: {}", input_file);
    let log = parse_bbl_file(Path::new(&input_file), export_opts.clone(), false)?;

    // Display log information
    println!("\nLog Information:");
    println!("  Total G frames (GPS): {}", log.stats.g_frames);
    println!("  Total H frames (Home): {}", log.stats.h_frames);
    println!("  GPS coordinates collected: {}", log.gps_coordinates.len());

    // Export GPS data if available
    if !log.gps_coordinates.is_empty() {
        println!("\nExporting to GPX...");
        export_to_gpx(
            Path::new(&input_file),
            0, // log index
            log.total_logs,
            &log.gps_coordinates,
            &log.home_coordinates,
            &export_opts,
        )?;
        println!("✓ GPX export complete");
        println!("  Exported {} GPS coordinates", log.gps_coordinates.len());
    } else {
        println!("\n⊘ No GPS coordinates available");
        println!("Note: This BBL file may not contain GPS data (no G-frames).");
        println!("Check that your flight controller has GPS enabled and connected.");
    }

    Ok(())
}
