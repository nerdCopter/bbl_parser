//! Event Export Example
//!
//! Demonstrates how to export flight event data to JSONL format.
//! Note: Event data collection requires the parser to populate event_frames.
//!       Currently, the parser module returns empty event vectors.
//!       Use the CLI for event export: `bbl_parser --event flight.BBL`

use bbl_parser::{export_to_event, parse_bbl_file, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    // Get input file from command line or show usage
    let input_file = std::env::args().nth(1).unwrap_or_else(|| {
        println!("Usage: event_export <input.BBL> [output_dir]");
        println!("Example: event_export flight.BBL ./output");
        println!("Note: Event export requires event data in the BBL file");
        std::process::exit(1);
    });

    // Get optional output directory from command line
    let output_dir = std::env::args().nth(2).map(|s| s.to_string());

    // Configure export options - Event export enabled
    let export_opts = ExportOptions {
        csv: false,
        gpx: false,
        event: true,
        output_dir: output_dir.clone(),
        force_export: false,
        store_all_frames: false, // No CSV export, memory efficient
    };

    // Parse the BBL file
    println!("Parsing: {}", input_file);
    let log = parse_bbl_file(Path::new(&input_file), export_opts.clone(), false)?;

    // Display log information
    println!("\nLog Information:");
    println!("  Total E frames (Events): {}", log.stats.e_frames);
    println!("  Event frames collected: {}", log.event_frames.len());

    // Export event data if available
    if !log.event_frames.is_empty() {
        println!("\nExporting to Event file...");
        export_to_event(
            Path::new(&input_file),
            0, // log index
            1, // total_logs (assuming single log for this example)
            &log.event_frames,
            &export_opts,
        )?;
        println!("✓ Event export complete");
        println!("  Exported {} events", log.event_frames.len());

        // Display sample events
        println!("\nEvent Summary:");
        for (i, event) in log.event_frames.iter().enumerate() {
            println!(
                "  {}. {} (time: {:.3}s)",
                i + 1,
                event.event_name,
                event.timestamp_us as f64 / 1_000_000.0
            );
        }
    } else {
        println!("\n⊘ No event data available");
        println!("Note: Event data collection in parser module not yet implemented.");
        println!("For event export, use the CLI: bbl_parser --event flight.BBL");
    }

    Ok(())
}
