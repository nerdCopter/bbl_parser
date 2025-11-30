use bbl_parser::{parse_bbl_file, ExportOptions};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let input_file = "./input/BTFL_KWONGKAN_10inch_0326_00_Filter.BBL";

    let export_opts = ExportOptions {
        csv: false,
        gpx: true,
        event: true,
        output_dir: Some("./output".to_string()),
        force_export: false,
    };

    println!("Parsing: {}", input_file);
    let log = parse_bbl_file(Path::new(input_file), export_opts, false)?;

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
