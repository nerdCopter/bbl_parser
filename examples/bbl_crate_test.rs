use anyhow::Result;
use bbl_parser::{parse_bbl_file_all_logs, ExportOptions};
use clap::Parser;
use glob::glob;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "bbl_crate_test")]
#[command(about = "Test program demonstrating BBL parser crate usage")]
struct Args {
    /// Input BBL files or glob patterns (case-insensitive)
    files: Vec<String>,
    
    /// Enable debug output
    #[arg(short, long)]
    debug: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    if args.files.is_empty() {
        eprintln!("Error: No input files specified");
        eprintln!("Usage: bbl_crate_test [OPTIONS] <FILES>...");
        eprintln!("Example: bbl_crate_test *.BBL *.bbl logs/*.{{BBL,BFL,TXT}}");
        std::process::exit(1);
    }
    
    let mut all_files = Vec::new();
    
    // Expand glob patterns and collect all matching files
    for pattern in &args.files {
        match glob(pattern) {
            Ok(paths) => {
                let mut found_files = false;
                for entry in paths {
                    match entry {
                        Ok(path) => {
                            if is_bbl_file(&path) {
                                all_files.push(path);
                                found_files = true;
                            }
                        }
                        Err(e) => eprintln!("Warning: Error reading path in pattern '{}': {}", pattern, e),
                    }
                }
                if !found_files {
                    // Try direct file access if glob didn't match
                    let path = PathBuf::from(pattern);
                    if path.exists() && is_bbl_file(&path) {
                        all_files.push(path);
                    } else if !path.exists() {
                        eprintln!("Warning: File not found: {}", pattern);
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Invalid glob pattern '{}': {}", pattern, e);
                // Try as direct file path
                let path = PathBuf::from(pattern);
                if path.exists() && is_bbl_file(&path) {
                    all_files.push(path);
                }
            }
        }
    }
    
    if all_files.is_empty() {
        eprintln!("Error: No valid BBL files found");
        std::process::exit(1);
    }
    
    // Sort files for consistent output
    all_files.sort();
    
    for file_path in all_files {
        process_file(&file_path, args.debug)?;
        println!();
    }
    
    Ok(())
}

/// Check if file has BBL-compatible extension (case-insensitive)
fn is_bbl_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        matches!(ext_str.as_str(), "bbl" | "bfl" | "txt")
    } else {
        false
    }
}

/// Process a single BBL file and display information
fn process_file(file_path: &Path, debug: bool) -> Result<()> {
    println!("Processing: {}", file_path.display());
    
    // Parse all logs in the file
    let export_options = ExportOptions::default(); // No file exports needed
    let logs = parse_bbl_file_all_logs(file_path, export_options, debug)?;
    
    for log in logs {
        if log.total_logs > 1 {
            println!("  Log {} of {}", log.log_number, log.total_logs);
        }
        
        // Basic flight information
        println!("  Firmware: {}", log.header.firmware_revision);
        println!("  Craft: {}", log.header.craft_name);
        
        // Flight duration calculation
        let duration = log.duration_seconds();
        println!("  Flight Duration: {:.1} seconds", duration);
        
        // PID settings from header (always shown)
        display_pid_settings(&log.header.all_headers);
        
        if log.total_logs > 1 {
            println!();
        }
    }
    
    Ok(())
}

/// Extract and display PID settings from system configuration
fn display_pid_settings(all_headers: &[String]) {
    println!("  PID Settings:");
    
    // First try to parse PID values with potential feedforward (4-value format for iNav)
    if let Some((roll_pid, pitch_pid, yaw_pid)) = parse_pid_with_ff_from_headers(all_headers) {
        // iNav 4-value format: P,I,D,FF
        println!("    Roll: P={}, I={}, D={}, FF={}", roll_pid.0, roll_pid.1, roll_pid.2, roll_pid.3);
        println!("    Pitch: P={}, I={}, D={}, FF={}", pitch_pid.0, pitch_pid.1, pitch_pid.2, pitch_pid.3);
        println!("    Yaw: P={}, I={}, D={}, FF={}", yaw_pid.0, yaw_pid.1, yaw_pid.2, yaw_pid.3);
    } else if let Some((roll_pid, pitch_pid, yaw_pid)) = parse_pid_from_headers(all_headers) {
        // Check if we have Betaflight feedforward values (ff_weight)
        if let Some((roll_ff, pitch_ff, yaw_ff)) = parse_feedforward_from_headers(all_headers) {
            // Betaflight: P,I,D from rollPID + FF from ff_weight
            println!("    Roll: P={}, I={}, D={}, FF={}", roll_pid.0, roll_pid.1, roll_pid.2, roll_ff);
            println!("    Pitch: P={}, I={}, D={}, FF={}", pitch_pid.0, pitch_pid.1, pitch_pid.2, pitch_ff);
            println!("    Yaw: P={}, I={}, D={}, FF={}", yaw_pid.0, yaw_pid.1, yaw_pid.2, yaw_ff);
        } else {
            // EmuFlight or older firmware: P,I,D only
            println!("    Roll: P={}, I={}, D={}", roll_pid.0, roll_pid.1, roll_pid.2);
            println!("    Pitch: P={}, I={}, D={}", pitch_pid.0, pitch_pid.1, pitch_pid.2);
            println!("    Yaw: P={}, I={}, D={}", yaw_pid.0, yaw_pid.1, yaw_pid.2);
        }
    } else {
        println!("    PID values not found in expected format");
    }
}

/// Parse PID values with feedforward from raw header lines (for iNav 4-value format)
fn parse_pid_with_ff_from_headers(all_headers: &[String]) -> Option<((i32, i32, i32, i32), (i32, i32, i32, i32), (i32, i32, i32, i32))> {
    let mut roll_pid = None;
    let mut pitch_pid = None;
    let mut yaw_pid = None;
    
    for header in all_headers {
        if header.starts_with("H rollPID:") {
            if let Some(value_str) = header.strip_prefix("H rollPID:") {
                roll_pid = parse_pid_with_ff_string(value_str.trim());
            }
        } else if header.starts_with("H pitchPID:") {
            if let Some(value_str) = header.strip_prefix("H pitchPID:") {
                pitch_pid = parse_pid_with_ff_string(value_str.trim());
            }
        } else if header.starts_with("H yawPID:") {
            if let Some(value_str) = header.strip_prefix("H yawPID:") {
                yaw_pid = parse_pid_with_ff_string(value_str.trim());
            }
        }
    }
    
    if let (Some(r), Some(p), Some(y)) = (roll_pid, pitch_pid, yaw_pid) {
        Some((r, p, y))
    } else {
        None
    }
}

/// Parse PID values from raw header lines (for Betaflight rollPID/pitchPID/yawPID format)
fn parse_pid_from_headers(all_headers: &[String]) -> Option<((i32, i32, i32), (i32, i32, i32), (i32, i32, i32))> {
    let mut roll_pid = None;
    let mut pitch_pid = None;
    let mut yaw_pid = None;
    
    for header in all_headers {
        if header.starts_with("H rollPID:") {
            if let Some(value_str) = header.strip_prefix("H rollPID:") {
                roll_pid = parse_pid_string(value_str.trim());
            }
        } else if header.starts_with("H pitchPID:") {
            if let Some(value_str) = header.strip_prefix("H pitchPID:") {
                pitch_pid = parse_pid_string(value_str.trim());
            }
        } else if header.starts_with("H yawPID:") {
            if let Some(value_str) = header.strip_prefix("H yawPID:") {
                yaw_pid = parse_pid_string(value_str.trim());
            }
        }
    }
    
    if let (Some(r), Some(p), Some(y)) = (roll_pid, pitch_pid, yaw_pid) {
        Some((r, p, y))
    } else {
        None
    }
}

/// Parse feedforward values from raw header lines (Betaflight ff_weight format)
fn parse_feedforward_from_headers(all_headers: &[String]) -> Option<(i32, i32, i32)> {
    for header in all_headers {
        if header.starts_with("H ff_weight:") {
            if let Some(value_str) = header.strip_prefix("H ff_weight:") {
                if let Some((roll_ff, pitch_ff, yaw_ff)) = parse_pid_string(value_str.trim()) {
                    return Some((roll_ff, pitch_ff, yaw_ff));
                }
            }
        }
    }
    None
}

/// Parse PID string in format "P,I,D,FF" and return (P, I, D, FF) tuple
fn parse_pid_with_ff_string(pid_value: &str) -> Option<(i32, i32, i32, i32)> {
    // Remove quotes if present
    let cleaned = pid_value.trim_matches('"');
    
    let parts: Vec<&str> = cleaned.split(',').collect();
    if parts.len() == 4 {
        if let (Ok(p), Ok(i), Ok(d), Ok(ff)) = (
            parts[0].trim().parse::<i32>(),
            parts[1].trim().parse::<i32>(),
            parts[2].trim().parse::<i32>(),
            parts[3].trim().parse::<i32>()
        ) {
            return Some((p, i, d, ff));
        }
    }
    None
}

/// Parse PID string in format "P,I,D" or "P,I,D,FF" and return (P, I, D) tuple
fn parse_pid_string(pid_value: &str) -> Option<(i32, i32, i32)> {
    // Remove quotes if present
    let cleaned = pid_value.trim_matches('"');
    
    let parts: Vec<&str> = cleaned.split(',').collect();
    if parts.len() >= 3 {
        if let (Ok(p), Ok(i), Ok(d)) = (
            parts[0].trim().parse::<i32>(),
            parts[1].trim().parse::<i32>(),
            parts[2].trim().parse::<i32>()
        ) {
            return Some((p, i, d));
            // Note: For iNav 4-value format "P,I,D,FF", we ignore the 4th value (feedforward)
        }
    }
    None
}
