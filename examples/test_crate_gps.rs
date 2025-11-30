use bbl_parser::{parse_bbl_file, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let input_file = std::env::args().nth(1).unwrap_or_else(|| {
        println!("Usage: test_crate_gps <input.BBL> [output_dir]");
        println!("Example: test_crate_gps flight.BBL ./output");
        println!("Note: Demonstrates GPS and Event frame parsing via the crate library");
        std::process::exit(1);
    });

    let output_dir = std::env::args().nth(2);

    let export_opts = ExportOptions {
        csv: false,
        gpx: true,
        event: true,
        output_dir,
        force_export: false,
    };

    println!("Parsing: {}", input_file);
    let log = parse_bbl_file(Path::new(&input_file), export_opts, false)?;

    println!("\nCrate Library Results:");
    println!("  Total frames: {}", log.stats.total_frames);
    println!("  I frames: {}", log.stats.i_frames);
    println!("  P frames: {}", log.stats.p_frames);
    println!("  G frames: {}", log.stats.g_frames);
    println!("  H frames: {}", log.stats.h_frames);
    println!("  E frames: {}", log.stats.e_frames);
    println!("  GPS coordinates collected: {}", log.gps_coordinates.len());
    println!(
        "  GPS home coords collected: {}",
        log.home_coordinates.len()
    );
    println!("  Event frames collected: {}", log.event_frames.len());

    Ok(())
}
