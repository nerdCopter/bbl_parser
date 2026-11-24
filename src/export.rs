//! Export functionality for BBL data
//!
//! Contains functions for exporting parsed BBL data to various formats
//! including CSV, GPX, and Event files.

use crate::conversion::*;
#[cfg(feature = "csv")]
use crate::firmware_tables::{
    DEFAULT_ACC_1G, DEFAULT_GYRO_SCALE, FIELDS_ACC_SMOOTH, FIELDS_GYRO_ADC, FIELD_AMPERAGE_LATEST,
    FIELD_BARO_ALT, FIELD_FAILSAFE_PHASE, FIELD_FLIGHT_MODE_FLAGS, FIELD_GPS_COORD,
    FIELD_GPS_GROUND_COURSE, FIELD_GPS_HOME, FIELD_GPS_SPEED, FIELD_STATE_FLAGS, FIELD_TIME,
    FIELD_VBAT_LATEST,
};
#[cfg(feature = "csv")]
use crate::parser::decoder::{PREDICT_HOME_COORD, PREDICT_LAST_MAIN_FRAME_TIME};
use crate::types::*;
#[cfg(feature = "csv")]
use crate::units::*;
#[cfg(feature = "csv")]
use anyhow::Context;
use anyhow::Result;
#[cfg(feature = "csv")]
use std::collections::HashMap;
use std::fs::File;
#[cfg(feature = "csv")]
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Export options for various output formats
///
/// Controls which export formats are generated and where files are written.
///
/// # Fields
/// - `csv`: Export flight data to CSV format (requires `csv` feature)
/// - `gpx`: Export GPS coordinates to GPX format for mapping
/// - `event`: Export events to JSON format
/// - `output_dir`: Optional custom output directory (defaults to input file's parent directory)
/// - `force_export`: Skip all filtering heuristics and always export
///
/// # Examples
/// ```rust
/// use bbl_parser::ExportOptions;
///
/// // Export everything to CSV with default location
/// let opts = ExportOptions {
///     csv: true,
///     gpx: false,
///     event: false,
///     output_dir: None,
///     force_export: false,
/// };
/// ```
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExportOptions {
    /// Enable CSV export of flight data
    pub csv: bool,
    /// Enable GPX export of GPS coordinates
    pub gpx: bool,
    /// Enable JSON export of flight events
    pub event: bool,
    /// Optional custom output directory (defaults to input file parent)
    pub output_dir: Option<String>,
    /// If true, export all logs without applying filtering heuristics
    pub force_export: bool,
}

/// Result of an export operation, containing paths of all files that were created.
///
/// Any path that is `None` indicates that export format was not requested, no data
/// was available for export (e.g., empty GPS coordinates for GPX export), or the
/// log was skipped by `should_skip_export` filtering heuristics.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExportReport {
    /// Path to the main CSV data file (None if CSV export was not performed)
    pub csv_path: Option<std::path::PathBuf>,
    /// Path to the CSV headers file (None if CSV export was not performed)
    pub headers_path: Option<std::path::PathBuf>,
    /// Path to the GPX file (None if GPX export was not performed or GPS data was empty)
    pub gpx_path: Option<std::path::PathBuf>,
    /// Path to the event JSON file (None if event export was not performed or no events were found)
    pub event_path: Option<std::path::PathBuf>,
    /// Reason the CSV export was skipped by `should_skip_export` filtering (None if not skipped)
    pub skip_reason: Option<String>,
}

/// Extract the base filename from an input path with consistent fallback.
/// Used by all export functions and path computation helpers to ensure
/// consistent naming across CSV, GPX, and event exports.
///
/// Always returns "blackbox" as fallback for missing or non-UTF-8 filenames,
/// ensuring compute_export_paths() predictions match actual export filenames.
fn extract_base_name(input_path: &Path) -> &str {
    input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("blackbox")
}

/// Sanitize a `base_name_override` value for safe use in file path construction.
/// Returns the final path component only, preventing directory traversal via `../`
/// segments or absolute paths. Returns `None` if the value is empty or ends in `..`.
fn sanitize_base_name_override(base_name_override: Option<&str>) -> Option<&str> {
    let raw = base_name_override?;
    Path::new(raw).file_name().and_then(|s| s.to_str())
}

/// Return a human-readable vendor name for a known filename prefix.
/// Falls back to `"Unknown"` for unrecognised prefixes.
pub fn vendor_name_for_prefix(prefix: &str) -> &'static str {
    match prefix {
        "EMUF_" => "EmuFlight",
        "BTFL_" => "Betaflight",
        "INAV_" => "iNav",
        "QUIC_" => "Quicksilver",
        _ => "Unknown",
    }
}

/// Known firmware vendor filename prefixes mapped to their revision keywords.
/// To add a new firmware: append `("PREFIX_", "keyword")` where keyword is a
/// lowercase substring of that firmware's `H Firmware revision:` header value.
/// For forks that share a base firmware's revision string, add an entry here
/// for filename detection and add a corresponding entry to `FORK_REVISION_MAP`.
const KNOWN_FIRMWARE_PREFIXES: &[(&str, &str)] = &[
    ("EMUF_", "emuflight"),
    ("BTFL_", "betaflight"),
    ("INAV_", "inav"),
    ("QUIC_", "quicksilver"), // Quicksilver; see FORK_REVISION_MAP for rename exemption
];

/// Fork firmware prefixes that intentionally report another vendor's revision string.
/// Sessions in files with these prefixes are NOT renamed even when the revision string
/// maps to a different canonical prefix — the mismatch is by design.
///
/// Each entry is `(file_prefix, reported_canonical_prefix)`.
/// To add a fork: append `("FORK_", "BASE_")` where BASE_ is what
/// `firmware_prefix_for_revision` returns for that fork's revision headers.
const FORK_REVISION_MAP: &[(&str, &str)] = &[
    // Quicksilver writes Betaflight revision headers for Blackbox Explorer compatibility.
    ("QUIC_", "BTFL_"),
];

/// Return the canonical filename prefix for a firmware revision string (e.g. `"BTFL_"`),
/// or `None` if the vendor is not recognised.
pub fn firmware_prefix_for_revision(revision: &str) -> Option<&'static str> {
    let rev_lower = revision.trim().to_lowercase();
    for &(prefix, keyword) in KNOWN_FIRMWARE_PREFIXES {
        if rev_lower.contains(keyword) {
            return Some(prefix);
        }
    }
    None
}

fn detect_bbl_filename_prefix(stem: &str) -> Option<&'static str> {
    KNOWN_FIRMWARE_PREFIXES
        .iter()
        .find(|&&(prefix, _)| stem.starts_with(prefix))
        .map(|&(prefix, _)| prefix)
}

/// Compute a corrected base name for a session whose firmware vendor differs from
/// the BBL filename prefix.  Returns `Some(corrected_stem)` when a replacement is
/// needed; `None` when the vendors match, either is unrecognised, or the file prefix
/// is a known fork that intentionally uses a different base firmware's revision string.
///
/// # Examples
/// - `EMUF_BLACKBOX_LOG_...BBL` + `EmuFlight 0.4.3`          → `None` (matches)
/// - `EMUF_BLACKBOX_LOG_...BBL` + `Betaflight 2025.12.0-beta` → `Some("BTFL_BLACKBOX_LOG_...")`
/// - `QUIC_Twiglet_...BFL`      + `Betaflight 4.3.0`          → `None` (fork exemption)
pub fn corrected_session_base_name(bbl_path: &Path, firmware_revision: &str) -> Option<String> {
    let stem = bbl_path.file_stem()?.to_str()?;
    let bbl_prefix = detect_bbl_filename_prefix(stem)?;
    let session_prefix = firmware_prefix_for_revision(firmware_revision)?;
    if bbl_prefix == session_prefix {
        return None;
    }
    // Known fork: the revision mismatch is by design — do not rename
    if FORK_REVISION_MAP
        .iter()
        .any(|&(fp, bp)| fp == bbl_prefix && bp == session_prefix)
    {
        return None;
    }
    Some(format!("{}{}", session_prefix, &stem[bbl_prefix.len()..]))
}

/// Helper to compute export file paths with consistent naming across all export types.
/// Ensures CLI status messages match actual filenames written by export functions.
///
/// # Arguments
/// * `input_path` - Path to the input BBL file (used to extract base filename)
/// * `export_options` - Export configuration with optional output directory
/// * `log_number` - 1-based log number (for .NN suffix when multiple logs)
/// * `total_logs` - Total number of logs in the file
/// * `base_name_override` - Optional replacement stem (e.g. from `corrected_session_base_name`)
///
/// # Returns
/// Tuple of (csv_path, headers_path, gpx_path, event_path) using consistent naming
pub fn compute_export_paths(
    input_path: &Path,
    export_options: &ExportOptions,
    log_number: usize,
    total_logs: usize,
    base_name_override: Option<&str>,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let base_name = sanitize_base_name_override(base_name_override)
        .unwrap_or_else(|| extract_base_name(input_path));

    let output_dir = if let Some(ref dir) = export_options.output_dir {
        std::path::Path::new(dir)
    } else {
        input_path.parent().unwrap_or(std::path::Path::new("."))
    };

    let log_suffix = if total_logs > 1 {
        format!(".{:02}", log_number)
    } else {
        String::new()
    };

    let csv_path = output_dir.join(format!("{}{}.csv", base_name, log_suffix));
    let headers_path = output_dir.join(format!("{}{}.headers.csv", base_name, log_suffix));
    let gpx_path = output_dir.join(format!("{}{}.gps.gpx", base_name, log_suffix));
    let event_path = output_dir.join(format!("{}{}.event", base_name, log_suffix));

    (csv_path, headers_path, gpx_path, event_path)
}

/// How one CSV column is produced.
#[cfg(feature = "csv")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldKind {
    Time,
    LoopIteration,
    Vbat,
    Amperage,
    Energy,
    Flags,
    Height,
    Gyro,
    Acc,
    Plain,
}

/// One CSV column: header text, source field name and how to write it.
#[cfg(feature = "csv")]
#[derive(Debug)]
struct CsvField {
    csv_name: String,
    lookup_name: String,
    kind: FieldKind,
}

/// Pre-computed CSV field mapping for performance
#[cfg(feature = "csv")]
#[derive(Debug)]
struct CsvFieldMap {
    fields: Vec<CsvField>,
}

/// Header text of a converted column: `name (unit)`, or `name` for raw.
#[cfg(feature = "csv")]
fn with_unit_label(name: &str, label: Option<&str>) -> String {
    match label {
        Some(label) => format!("{name} ({label})"),
        None => name.to_string(),
    }
}

#[cfg(feature = "csv")]
impl CsvFieldMap {
    fn new(header: &BBLHeader, units: &UnitOptions) -> Self {
        let mut fields = Vec::new();

        // I frame fields
        for field_name in &header.i_frame_def.field_names {
            let trimmed = field_name.trim();
            let (csv_name, kind) = if trimmed == FIELD_TIME {
                (
                    with_unit_label(trimmed, units.frame_time.label()),
                    FieldKind::Time,
                )
            } else if trimmed == FIELD_VBAT_LATEST {
                (
                    with_unit_label(trimmed, units.vbat.label()),
                    FieldKind::Vbat,
                )
            } else if trimmed == FIELD_AMPERAGE_LATEST {
                (
                    with_unit_label(trimmed, units.amperage.label()),
                    FieldKind::Amperage,
                )
            } else if trimmed == FIELD_BARO_ALT && units.height.is_some() {
                (
                    with_unit_label(trimmed, units.height.and_then(HeightUnit::label)),
                    FieldKind::Height,
                )
            } else if FIELDS_GYRO_ADC.contains(&trimmed) {
                (
                    with_unit_label(trimmed, units.rotation.label()),
                    FieldKind::Gyro,
                )
            } else if FIELDS_ACC_SMOOTH.contains(&trimmed) {
                (
                    with_unit_label(trimmed, units.acceleration.label()),
                    FieldKind::Acc,
                )
            } else if trimmed == "loopIteration" {
                (trimmed.to_string(), FieldKind::LoopIteration)
            } else {
                (trimmed.to_string(), FieldKind::Plain)
            };

            fields.push(CsvField {
                csv_name,
                lookup_name: trimmed.to_string(),
                kind,
            });
        }

        // Add computed fields IMMEDIATELY after I frame fields (like blackbox_decode does)
        if fields
            .iter()
            .any(|field| field.lookup_name == FIELD_AMPERAGE_LATEST)
        {
            fields.push(CsvField {
                csv_name: "energyCumulative (mAh)".to_string(),
                lookup_name: String::new(),
                kind: FieldKind::Energy,
            });
        }

        // S frame fields (with flag formatting)
        for field_name in &header.s_frame_def.field_names {
            let trimmed = field_name.trim();
            if trimmed == FIELD_TIME {
                continue;
            } // Skip duplicate

            let is_flags = trimmed.contains("Flag") || trimmed == FIELD_FAILSAFE_PHASE;
            let (csv_name, kind) = if is_flags {
                (
                    with_unit_label(trimmed, units.flags.label().filter(|_| !units.raw)),
                    FieldKind::Flags,
                )
            } else {
                (trimmed.to_string(), FieldKind::Plain)
            };

            fields.push(CsvField {
                csv_name,
                lookup_name: trimmed.to_string(),
                kind,
            });
        }

        Self { fields }
    }
}

/// Export BBL log to CSV format
///
/// Applies the same low-value-flight filtering heuristics as the CLI
/// (see [`crate::filters::should_skip_export`]), gated by
/// `export_options.force_export`. A skipped log returns an `ExportReport`
/// with all paths `None` and `skip_reason` set, rather than an error.
///
/// # Returns
/// An `ExportReport` containing paths to the CSV and headers files that were created,
/// or an error if the export failed.
#[cfg(feature = "csv")]
pub fn export_to_csv(
    log: &BBLLog,
    input_path: &Path,
    export_options: &ExportOptions,
    base_name_override: Option<&str>,
) -> Result<ExportReport> {
    export_to_csv_with_units(
        log,
        input_path,
        export_options,
        base_name_override,
        &UnitOptions::default(),
    )
}

/// Like [`export_to_csv`], with the output units chosen by `units`.
///
/// The headers file is not affected by `units`.
#[cfg(feature = "csv")]
pub fn export_to_csv_with_units(
    log: &BBLLog,
    input_path: &Path,
    export_options: &ExportOptions,
    base_name_override: Option<&str>,
    units: &UnitOptions,
) -> Result<ExportReport> {
    let (should_skip, reason) =
        crate::filters::should_skip_export(log, export_options.force_export);
    if should_skip {
        return Ok(ExportReport {
            skip_reason: Some(reason),
            ..ExportReport::default()
        });
    }

    let base_name = sanitize_base_name_override(base_name_override)
        .unwrap_or_else(|| extract_base_name(input_path));

    let output_dir = if let Some(ref dir) = export_options.output_dir {
        Path::new(dir)
    } else {
        input_path.parent().unwrap_or(Path::new("."))
    };

    // Create output directory if it doesn't exist
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)?;
    }

    let log_suffix = if log.total_logs > 1 {
        format!(".{:02}", log.log_number)
    } else {
        "".to_string()
    };

    // Export plaintext headers to separate CSV
    let header_csv_path = output_dir.join(format!("{base_name}{log_suffix}.headers.csv"));
    export_headers_to_csv(&log.header, &header_csv_path)?;

    // Export flight data (I, P, S frames) to main CSV
    let flight_csv_path = output_dir.join(format!("{base_name}{log_suffix}.csv"));
    export_flight_data_to_csv(log, &flight_csv_path, units)?;

    Ok(ExportReport {
        csv_path: Some(flight_csv_path),
        headers_path: Some(header_csv_path),
        gpx_path: None,
        event_path: None,
        skip_reason: None,
    })
}

/// Export headers to CSV file
#[cfg(feature = "csv")]
fn export_headers_to_csv(header: &BBLHeader, output_path: &Path) -> Result<()> {
    let file = File::create(output_path)
        .with_context(|| format!("Failed to create headers CSV file: {output_path:?}"))?;
    let mut writer = BufWriter::new(file);

    // Write CSV header
    writeln!(writer, "Field,Value")?;

    // Parse and write all header lines
    for header_line in &header.all_headers {
        if let Some(content) = header_line.strip_prefix("H ") {
            // Remove "H " prefix and find the colon separator
            if let Some(colon_pos) = content.find(':') {
                let field_name = content[..colon_pos].trim();
                let field_value = content[colon_pos + 1..].trim();

                // Escape commas in values by wrapping in quotes
                let escaped_value = if field_value.contains(',') {
                    format!("\"{}\"", field_value.replace('"', "\"\""))
                } else {
                    field_value.to_string()
                };

                writeln!(writer, "{field_name},{escaped_value}")?;
            }
        }
    }

    writer
        .flush()
        .with_context(|| format!("Failed to flush headers CSV file: {output_path:?}"))?;

    Ok(())
}

/// Export flight data to CSV file
#[cfg(feature = "csv")]
fn export_flight_data_to_csv(log: &BBLLog, output_path: &Path, units: &UnitOptions) -> Result<()> {
    let file = File::create(output_path)
        .with_context(|| format!("Failed to create flight data CSV file: {output_path:?}"))?;
    let mut writer = BufWriter::new(file);

    export_flight_data_to_writer_with_units(log, &mut writer, units)?;

    writer
        .flush()
        .with_context(|| format!("Failed to flush flight data CSV file: {output_path:?}"))?;

    Ok(())
}

/// Write the flight data (I, P, S frames) of `log` as CSV to any writer.
///
/// Produces exactly the bytes of the main `.csv` file written by
/// [`export_to_csv`], so a caller can stream to stdout or memory instead of a
/// file. No filtering heuristics are applied, and the writer is not flushed:
/// the caller must flush a buffered writer. Nothing is written when the log
/// has no I/P frames.
#[cfg(feature = "csv")]
pub fn export_flight_data_to_writer<W: Write>(log: &BBLLog, writer: &mut W) -> Result<()> {
    export_flight_data_to_writer_with_units(log, writer, &UnitOptions::default())
}

/// Like [`export_flight_data_to_writer`], with the output units chosen by `units`.
///
/// `UnitOptions::default()` gives exactly the output of
/// [`export_flight_data_to_writer`].
#[cfg(feature = "csv")]
pub fn export_flight_data_to_writer_with_units<W: Write>(
    log: &BBLLog,
    writer: &mut W,
    units: &UnitOptions,
) -> Result<()> {
    // Build optimized field mapping
    let csv_map = CsvFieldMap::new(&log.header, units);
    let field_names: Vec<&str> = csv_map
        .fields
        .iter()
        .map(|field| field.csv_name.as_str())
        .collect();
    let gyro_scale = gyro_scale_radians_per_microsecond(&log.header).unwrap_or(DEFAULT_GYRO_SCALE);
    let acc_1g = acc_1g(&log.header).unwrap_or(DEFAULT_ACC_1G);

    let g_names: Vec<&str> = if units.merge_gps {
        log.header
            .g_frame_def
            .field_names
            .iter()
            .map(|name| name.trim())
            .filter(|name| *name != FIELD_TIME)
            .collect()
    } else {
        Vec::new()
    };
    let rows = collect_csv_rows(log, !g_names.is_empty(), units.raw);

    if rows.is_empty() {
        return Ok(()); // No data to export
    }

    // Write field names header
    for (i, field_name) in field_names.iter().enumerate() {
        if i > 0 {
            write!(writer, ", ")?;
        }
        write!(writer, "{field_name}")?;
    }
    for name in &g_names {
        write!(writer, ", ")?;
        if *name == FIELD_GPS_SPEED {
            write!(
                writer,
                "{}",
                with_unit_label(name, units.gps_speed.label().filter(|_| !units.raw))
            )?;
        } else {
            write!(writer, "{name}")?;
        }
    }
    writeln!(writer)?;

    // Optimized CSV writing with pre-computed mappings
    let mut cumulative_energy_mah = 0f32;
    let mut last_timestamp_us = 0u64;
    let mut latest_s_frame_data: HashMap<String, i32> = HashMap::new();

    for (output_iteration, row) in rows.iter().enumerate() {
        let frame = row.frame;
        let timestamp = &row.timestamp;
        // Update latest S-frame data if this is an S frame
        if frame.frame_type == 'S' {
            for (key, value) in &frame.data {
                latest_s_frame_data.insert(key.clone(), *value);
            }
        }

        // Calculate energyCumulative for this frame; a row the GPS merge
        // repeats for a G frame adds no new current sample.
        if let Some(current_raw) = frame.data.get("amperageLatest").copied() {
            if row.new_sample {
                if last_timestamp_us > 0 && *timestamp > last_timestamp_us {
                    let time_delta_hours =
                        (*timestamp - last_timestamp_us) as f32 / 3_600_000_000.0;
                    let current_amps = convert_amperage_to_amps(current_raw);
                    cumulative_energy_mah += current_amps * time_delta_hours * 1000.0;
                }
                last_timestamp_us = *timestamp;
            }
        }

        // Write data row using optimized field mapping
        for (i, field) in csv_map.fields.iter().enumerate() {
            if i > 0 {
                write!(writer, ", ")?;
            }

            let lookup_name = &field.lookup_name;
            let raw_of = |name: &str| frame.data.get(name).copied().unwrap_or(0);
            let raw_or_slow = |name: &str| {
                frame
                    .data
                    .get(name)
                    .copied()
                    .or_else(|| latest_s_frame_data.get(name).copied())
                    .unwrap_or(0)
            };

            match field.kind {
                FieldKind::Time => match units.frame_time {
                    FrameTimeUnit::Microseconds => write!(writer, "{}", *timestamp as i32)?,
                    FrameTimeUnit::Raw => write!(writer, "{:3}", *timestamp as i32)?,
                    FrameTimeUnit::Milliseconds => {
                        write!(writer, "{:.3}", *timestamp as f64 / 1000.0)?
                    }
                    FrameTimeUnit::Seconds => {
                        write!(writer, "{:.6}", *timestamp as f64 / 1_000_000.0)?
                    }
                },
                FieldKind::LoopIteration => {
                    let value = frame
                        .data
                        .get("loopIteration")
                        .copied()
                        .unwrap_or(output_iteration as i32);
                    write!(writer, "{value:4}")?;
                }
                FieldKind::Vbat => {
                    let raw_value = raw_of(lookup_name);
                    let volts = convert_vbat_to_volts(raw_value, &log.header.firmware_revision);
                    match units.vbat {
                        VbatUnit::Volts => write!(writer, "{volts:4.1}")?,
                        VbatUnit::Raw => write!(writer, "{raw_value:3}")?,
                        VbatUnit::MilliVolts => {
                            write!(writer, "{:3}", (volts as f64 * 1000.0).round() as i64)?
                        }
                    }
                }
                FieldKind::Amperage => {
                    let raw_value = raw_of(lookup_name);
                    match units.amperage {
                        AmperageUnit::Amps => {
                            write!(writer, "{:4.2}", convert_amperage_to_amps(raw_value))?
                        }
                        AmperageUnit::Raw => write!(writer, "{raw_value:3}")?,
                        AmperageUnit::MilliAmps => write!(writer, "{:3}", raw_value as i64 * 10)?,
                    }
                }
                FieldKind::Energy => write!(writer, "{:5}", cumulative_energy_mah as i32)?,
                FieldKind::Flags => {
                    // Handle flag fields - output text values like blackbox_decode.c
                    let raw_value = raw_or_slow(lookup_name);

                    if units.flags == FlagsUnit::Raw {
                        write!(writer, "{}", raw_value as u32)?;
                    } else {
                        let formatted = if lookup_name == FIELD_FLIGHT_MODE_FLAGS {
                            format_flight_mode_flags(raw_value)
                        } else if lookup_name == FIELD_STATE_FLAGS {
                            format_state_flags(raw_value)
                        } else if lookup_name == FIELD_FAILSAFE_PHASE {
                            format_failsafe_phase(raw_value)
                        } else {
                            raw_value.to_string()
                        };
                        write!(writer, "{formatted}")?;
                    }
                }
                FieldKind::Height => {
                    let raw_value = raw_or_slow(lookup_name);
                    match units.height {
                        Some(HeightUnit::Centimeters) => write!(writer, "{raw_value}")?,
                        Some(HeightUnit::Raw) | None => write!(writer, "{raw_value:3}")?,
                        Some(unit) => write!(writer, "{:.2}", height_to_unit(raw_value, unit))?,
                    }
                }
                FieldKind::Gyro => {
                    let raw_value = raw_or_slow(lookup_name);
                    match units.rotation {
                        RotationUnit::Raw => write!(writer, "{raw_value:4}")?,
                        unit => write!(writer, "{:.2}", gyro_to_unit(raw_value, gyro_scale, unit))?,
                    }
                }
                FieldKind::Acc => {
                    let raw_value = raw_or_slow(lookup_name);
                    match units.acceleration {
                        AccelerationUnit::Raw => write!(writer, "{raw_value:4}")?,
                        unit => write!(
                            writer,
                            "{:.2}",
                            acceleration_to_unit(raw_value, acc_1g, unit)
                        )?,
                    }
                }
                FieldKind::Plain => {
                    // Regular field lookup with S-frame fallback
                    let value = raw_or_slow(lookup_name);
                    write!(writer, "{value:4}")?;
                }
            }
        }
        for name in &g_names {
            write!(writer, ", ")?;
            let raw = row
                .gps
                .and_then(|g| g.data.get(*name).copied())
                .unwrap_or(0);
            if FIELD_GPS_COORD.contains(name) {
                let home_raw = row.home[usize::from(*name != FIELD_GPS_COORD[0])];
                // Before the first G frame blackbox_decode writes plain zeros.
                let absolute = if row.gps.is_some() {
                    raw as i64 + home_raw
                } else {
                    0
                };
                write!(writer, "{}", format_gps_coordinate(absolute))?;
            } else if *name == FIELD_GPS_SPEED {
                write!(writer, "{}", format_gps_speed(raw, units.gps_speed))?;
            } else if *name == FIELD_GPS_GROUND_COURSE {
                write!(writer, "{}", format_gps_course(raw))?;
            } else {
                write!(writer, "{raw}")?;
            }
        }
        writeln!(writer)?;
    }

    Ok(())
}

/// One flight-data CSV row.
#[cfg(feature = "csv")]
struct CsvRow<'a> {
    timestamp: u64,
    frame: &'a DecodedFrame,
    /// Latest G frame when the GPS fields are merged.
    gps: Option<&'a DecodedFrame>,
    /// Home position (degrees times 10^7) of the latest H frame.
    home: [i64; 2],
    /// False for the row a G frame repeats of the previous main frame.
    new_sample: bool,
}

/// Order the I and P frames into CSV rows. With `merge_gps`, follow
/// `onFrameReadyMerge` in blackbox_decode: a main frame waits for a possible
/// G frame at the same time and is written with it; a G frame at another time
/// first flushes the waiting frame, then adds a row of its own that repeats
/// the last main frame under the G frame time.
#[cfg(feature = "csv")]
fn collect_csv_rows(log: &BBLLog, merge_gps: bool, stream_order: bool) -> Vec<CsvRow<'_>> {
    if !merge_gps {
        let mut rows: Vec<CsvRow<'_>> = log
            .frames
            .iter()
            .filter(|f| f.frame_type == 'I' || f.frame_type == 'P')
            .map(|frame| CsvRow {
                timestamp: frame.timestamp_us,
                frame,
                gps: None,
                home: [0, 0],
                new_sample: true,
            })
            .collect();
        if !stream_order {
            rows.sort_by_key(|row| row.timestamp);
        }
        return rows;
    }

    let g_def = &log.header.g_frame_def;
    let predictor_of = |name: &str| {
        g_def
            .field_names
            .iter()
            .position(|n| n.trim() == name)
            .and_then(|i| g_def.fields.get(i))
            .map(|f| f.predictor)
    };
    // --raw runs no predictor, so offsets stay offsets.
    let applies =
        |name: &str, predictor: u8| !stream_order && predictor_of(name) == Some(predictor);
    let time_is_offset = applies(FIELD_TIME, PREDICT_LAST_MAIN_FRAME_TIME);
    let coord_has_home = [
        applies(FIELD_GPS_COORD[0], PREDICT_HOME_COORD),
        applies(FIELD_GPS_COORD[1], PREDICT_HOME_COORD),
    ];

    let mut rows = Vec::new();
    let mut waiting: Option<&DecodedFrame> = None;
    let mut last_main: Option<&DecodedFrame> = None;
    let mut last_time: Option<u64> = None;
    let mut gps: Option<&DecodedFrame> = None;
    // Home position added to the coordinates of `gps`, fixed when that G frame was read.
    let mut gps_home = [0i64; 2];
    // blackbox_decode drops G frames until an H (home) frame has been read.
    let mut home: Option<[i64; 2]> = None;
    for frame in &log.frames {
        match frame.frame_type {
            'H' => {
                let coord = |name: &str| frame.data.get(name).copied().unwrap_or(0) as i64;
                home = Some([coord(FIELD_GPS_HOME[0]), coord(FIELD_GPS_HOME[1])]);
            }
            'I' | 'P' => {
                if let Some(main) = waiting.take() {
                    rows.push(CsvRow {
                        timestamp: main.timestamp_us,
                        frame: main,
                        gps,
                        home: gps_home,
                        new_sample: true,
                    });
                }
                waiting = Some(frame);
                last_main = Some(frame);
                last_time = Some(frame.timestamp_us);
            }
            'G' if home.is_some() => {
                // A G frame without a time field belongs to the last main frame.
                let gps_time = match (frame.data.get(FIELD_TIME), last_time) {
                    (Some(offset), Some(base)) if time_is_offset => {
                        base.wrapping_add_signed(*offset as i64)
                    }
                    (Some(time), _) => *time as u64,
                    _ => last_time.unwrap_or(frame.timestamp_us),
                };
                if last_time != Some(gps_time) {
                    if let Some(main) = waiting.take() {
                        rows.push(CsvRow {
                            timestamp: main.timestamp_us,
                            frame: main,
                            gps,
                            home: gps_home,
                            new_sample: true,
                        });
                    }
                }
                gps = Some(frame);
                let home_pos = home.unwrap_or_default();
                gps_home = [
                    if coord_has_home[0] { home_pos[0] } else { 0 },
                    if coord_has_home[1] { home_pos[1] } else { 0 },
                ];
                let new_sample = waiting.is_some();
                if let Some(main) = waiting.take().or(last_main) {
                    rows.push(CsvRow {
                        timestamp: gps_time,
                        frame: main,
                        gps,
                        home: gps_home,
                        new_sample,
                    });
                }
            }
            _ => {}
        }
    }
    if let Some(main) = waiting {
        rows.push(CsvRow {
            timestamp: main.timestamp_us,
            frame: main,
            gps,
            home: gps_home,
            new_sample: true,
        });
    }
    rows
}

/// Export GPS data to GPX format
///
/// # Arguments
/// * `input_path` - Path to the input BBL file (used for output naming)
/// * `log_index` - Index of the current log (0-based)
/// * `total_logs` - Total number of logs in the file
/// * `gps_coordinates` - GPS coordinate data to export
/// * `home_coordinates` - Home coordinates from H frames (used for home waypoint marker)
/// * `export_options` - Export configuration options
/// * `log_start_datetime` - Optional log start datetime from header for accurate timestamps
///
/// # Features
/// When home coordinates are available, adds a home position waypoint to the GPX file.
/// This provides a visual reference point in GPS mapping tools.
///
/// # Performance Notes
/// For very large GPS traces, the `log_start_datetime` is parsed via `generate_gpx_timestamp()`
/// on each trackpoint. Future optimization: consider caching the parsed base epoch once per log
/// to avoid repeated parsing overhead when exporting thousands of GPS points.
#[allow(clippy::too_many_arguments)]
pub fn export_to_gpx(
    input_path: &Path,
    log_index: usize,
    total_logs: usize,
    gps_coordinates: &[GpsCoordinate],
    home_coordinates: &[GpsHomeCoordinate],
    export_options: &ExportOptions,
    log_start_datetime: Option<&str>,
    base_name_override: Option<&str>,
) -> Result<ExportReport> {
    export_to_gpx_with_alt_offset(
        input_path,
        log_index,
        total_logs,
        gps_coordinates,
        home_coordinates,
        export_options,
        log_start_datetime,
        base_name_override,
        0.0,
    )
}

/// Like [`export_to_gpx`], adding `alt_offset_m` meters to every track point's
/// `<ele>` (`blackbox_decode --alt-offset`). The home waypoint has no elevation.
#[allow(clippy::too_many_arguments)]
pub fn export_to_gpx_with_alt_offset(
    input_path: &Path,
    log_index: usize,
    total_logs: usize,
    gps_coordinates: &[GpsCoordinate],
    home_coordinates: &[GpsHomeCoordinate],
    export_options: &ExportOptions,
    log_start_datetime: Option<&str>,
    base_name_override: Option<&str>,
    alt_offset_m: f64,
) -> Result<ExportReport> {
    if gps_coordinates.is_empty() {
        return Ok(ExportReport::default());
    }

    // Use compute_export_paths to ensure consistent naming with CSV exports
    let (_, _, gpx_path, _) = compute_export_paths(
        input_path,
        export_options,
        log_index + 1,
        total_logs,
        base_name_override,
    );

    // Create output directory if it doesn't exist (match export_to_csv behavior)
    if let Some(parent) = gpx_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut gpx_file = File::create(&gpx_path)?;
    writeln!(gpx_file, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        gpx_file,
        r#"<gpx creator="BBL Parser (Rust)" version="1.1" xmlns="http://www.topografix.com/GPX/1/1" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd">"#
    )?;
    writeln!(
        gpx_file,
        "<metadata><name>Blackbox flight log</name></metadata>"
    )?;

    // Add home position waypoint if available
    if let Some(home) = home_coordinates.first() {
        writeln!(
            gpx_file,
            r#"  <wpt lat="{:.7}" lon="{:.7}">"#,
            home.home_latitude, home.home_longitude
        )?;
        writeln!(gpx_file, r#"    <name>Home</name>"#)?;
        writeln!(gpx_file, r#"    <sym>Flag</sym>"#)?;
        writeln!(gpx_file, r#"    <desc>Home Position</desc>"#)?;
        writeln!(gpx_file, r#"  </wpt>"#)?;
    }

    writeln!(gpx_file, "<trk><name>Blackbox flight log</name><trkseg>")?;

    for coord in gps_coordinates {
        // Only include coordinates with sufficient GPS satellite count (minimum 5)
        if let Some(num_sats) = coord.num_sats {
            if num_sats < 5 {
                continue;
            }
        }

        // Generate GPX timestamp from log_start_datetime + frame timestamp
        // Following blackbox_decode approach: dateTime + (gpsFrameTime / 1000000)
        let timestamp_str = generate_gpx_timestamp(log_start_datetime, coord.timestamp_us);

        writeln!(
            gpx_file,
            r#"  <trkpt lat="{:.7}" lon="{:.7}"><ele>{:.2}</ele><time>{}</time></trkpt>"#,
            coord.latitude,
            coord.longitude,
            coord.altitude + alt_offset_m,
            timestamp_str
        )?;
    }

    writeln!(gpx_file, "</trkseg></trk>")?;
    writeln!(gpx_file, "</gpx>")?;

    Ok(ExportReport {
        csv_path: None,
        headers_path: None,
        gpx_path: Some(gpx_path),
        event_path: None,
        skip_reason: None,
    })
}

/// Export event data to file
///
/// # Returns
/// An `ExportReport` containing the path to the event file that was created,
/// or an error if the export failed. Returns `None` for `event_path` if no events were exported.
pub fn export_to_event(
    input_path: &Path,
    log_index: usize,
    total_logs: usize,
    event_frames: &[EventFrame],
    export_options: &ExportOptions,
    base_name_override: Option<&str>,
) -> Result<ExportReport> {
    if event_frames.is_empty() {
        return Ok(ExportReport::default());
    }

    // Use compute_export_paths to ensure consistent naming with CSV exports
    let (_, _, _, event_path) = compute_export_paths(
        input_path,
        export_options,
        log_index + 1,
        total_logs,
        base_name_override,
    );

    // Create output directory if it doesn't exist (match export_to_csv behavior)
    if let Some(parent) = event_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut event_file = File::create(&event_path)?;

    // Export as JSONL format (individual JSON objects per line) to match blackbox_decode
    for event in event_frames.iter() {
        writeln!(
            event_file,
            r#"{{"name":"{}", "time":{}}}"#,
            event.event_name.replace('"', "\\\""),
            event.timestamp_us
        )?;
    }

    Ok(ExportReport {
        csv_path: None,
        headers_path: None,
        gpx_path: None,
        event_path: Some(event_path),
        skip_reason: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::TempDir;

    /// Test helper to create a minimal GPX export and read back the content
    fn export_gpx_and_read(
        gps_coords: &[GpsCoordinate],
        home_coords: &[GpsHomeCoordinate],
    ) -> Result<String> {
        let temp_dir = TempDir::new()?;
        let temp_input_path = temp_dir.path().join("test_input.bbl");

        let export_opts = ExportOptions {
            csv: false,
            gpx: true,
            event: false,
            output_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
            force_export: false,
        };

        export_to_gpx(
            &temp_input_path,
            0,
            1,
            gps_coords,
            home_coords,
            &export_opts,
            None,
            None,
        )?;

        // Read back the generated GPX file
        let gpx_path = temp_dir.path().join("test_input.gps.gpx");
        let mut gpx_content = String::new();
        let mut gpx_file = File::open(&gpx_path)?;
        gpx_file.read_to_string(&mut gpx_content)?;

        Ok(gpx_content)
    }

    #[test]
    fn test_gpx_home_waypoint_with_coordinates() -> Result<()> {
        let home_coords = vec![GpsHomeCoordinate {
            home_latitude: 40.7128,
            home_longitude: -74.0060,
            timestamp_us: 0,
        }];

        let gps_coords = vec![GpsCoordinate {
            latitude: 40.7129,
            longitude: -74.0061,
            altitude: 100.0,
            timestamp_us: 1_000_000,
            num_sats: Some(10),
            speed: Some(5.0),
            ground_course: Some(180.0),
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify home waypoint element exists
        assert!(
            content.contains(r#"<wpt lat="40.7128000" lon="-74.0060000">"#),
            "Home waypoint coordinates should be formatted to 7 decimal places"
        );

        // Verify home waypoint has correct structure
        assert!(
            content.contains("<name>Home</name>"),
            "Home waypoint should have <name>Home</name>"
        );
        assert!(
            content.contains("<sym>Flag</sym>"),
            "Home waypoint should have <sym>Flag</sym>"
        );
        assert!(
            content.contains("<desc>Home Position</desc>"),
            "Home waypoint should have <desc>Home Position</desc>"
        );

        // Verify closing tag
        assert!(
            content.contains("</wpt>"),
            "Home waypoint should have closing tag"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_home_waypoint_precision() -> Result<()> {
        let home_coords = vec![GpsHomeCoordinate {
            home_latitude: 51.5074123456789,
            home_longitude: -0.1278123456789,
            timestamp_us: 0,
        }];

        let gps_coords = vec![GpsCoordinate {
            latitude: 51.5075,
            longitude: -0.1280,
            altitude: 50.0,
            timestamp_us: 1_000_000,
            num_sats: Some(5),
            speed: None,
            ground_course: None,
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify coordinates are truncated/rounded to 7 decimal places
        assert!(
            content.contains(r#"lat="51.5074123""#),
            "Latitude should be formatted to 7 decimal places (truncated)"
        );
        assert!(
            content.contains(r#"lon="-0.1278123""#),
            "Longitude should be formatted to 7 decimal places (truncated)"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_no_home_waypoint_when_empty() -> Result<()> {
        let home_coords = vec![];

        let gps_coords = vec![GpsCoordinate {
            latitude: 40.7129,
            longitude: -74.0061,
            altitude: 100.0,
            timestamp_us: 1_000_000,
            num_sats: Some(10),
            speed: Some(5.0),
            ground_course: Some(180.0),
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify no home waypoint appears when home_coordinates is empty
        assert!(
            !content.contains("<name>Home</name>"),
            "No home waypoint should be present when home_coordinates is empty"
        );
        assert!(
            !content.contains("<wpt"),
            "No waypoint element should be present when home_coordinates is empty"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_home_waypoint_xml_structure() -> Result<()> {
        let home_coords = vec![GpsHomeCoordinate {
            home_latitude: 35.6762,
            home_longitude: 139.6503,
            timestamp_us: 0,
        }];

        let gps_coords = vec![GpsCoordinate {
            latitude: 35.6763,
            longitude: 139.6504,
            altitude: 25.0,
            timestamp_us: 1_000_000,
            num_sats: Some(8),
            speed: Some(2.0),
            ground_course: Some(45.0),
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify basic GPX structure
        assert!(
            content.contains(r#"<?xml version="1.0" encoding="UTF-8"?>"#),
            "XML declaration should be present"
        );
        assert!(
            content.contains("version=\"1.1\""),
            "GPX version should be 1.1"
        );
        assert!(
            content.contains("xmlns=\"http://www.topografix.com/GPX/1/1\""),
            "GPX namespace should be present"
        );

        // Verify home waypoint appears before track element
        let wpt_index = content.find("<wpt").expect("Should have <wpt element");
        let trk_index = content.find("<trk").expect("Should have <trk element");
        assert!(
            wpt_index < trk_index,
            "Home waypoint should appear before track element in GPX file"
        );

        // Verify proper nesting and indentation
        assert!(
            content.contains("  <wpt"),
            "Home waypoint should be properly indented"
        );
        assert!(
            content.contains("    <name>Home</name>"),
            "Home waypoint child elements should be properly indented"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_home_waypoint_with_negative_coordinates() -> Result<()> {
        let home_coords = vec![GpsHomeCoordinate {
            home_latitude: -33.8688,
            home_longitude: 151.2093,
            timestamp_us: 0,
        }];

        let gps_coords = vec![GpsCoordinate {
            latitude: -33.8689,
            longitude: 151.2094,
            altitude: 150.0,
            timestamp_us: 1_000_000,
            num_sats: Some(12),
            speed: Some(10.0),
            ground_course: Some(270.0),
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify negative coordinates are correctly formatted
        assert!(
            content.contains(r#"lat="-33.8688000""#),
            "Negative latitude should be correctly formatted"
        );
        assert!(
            content.contains(r#"lon="151.2093000""#),
            "Positive longitude should be correctly formatted"
        );

        // Verify structure is still correct with negative values
        assert!(
            content.contains("<name>Home</name>"),
            "Structure should be correct"
        );
        assert!(content.contains("</wpt>"), "Closing tag should be present");

        Ok(())
    }

    #[test]
    fn test_gpx_only_first_home_coordinate_used() -> Result<()> {
        let home_coords = vec![
            GpsHomeCoordinate {
                home_latitude: 40.7128,
                home_longitude: -74.0060,
                timestamp_us: 0,
            },
            GpsHomeCoordinate {
                home_latitude: 51.5074,
                home_longitude: -0.1278,
                timestamp_us: 1_000_000,
            },
        ];

        let gps_coords = vec![GpsCoordinate {
            latitude: 40.7129,
            longitude: -74.0061,
            altitude: 100.0,
            timestamp_us: 1_000_000,
            num_sats: Some(10),
            speed: Some(5.0),
            ground_course: Some(180.0),
        }];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify only first coordinate is used
        assert!(
            content.contains(r#"lat="40.7128000""#),
            "First home coordinate should be used"
        );
        // Count occurrences of <wpt to ensure only one home waypoint
        let wpt_count = content.matches("<wpt").count();
        assert_eq!(
            wpt_count, 1,
            "Only one home waypoint should be present when multiple home_coordinates exist"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_empty_gps_coordinates_returns_ok() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let temp_input_path = temp_dir.path().join("test_input.bbl");

        let export_opts = ExportOptions {
            csv: false,
            gpx: true,
            event: false,
            output_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
            force_export: false,
        };

        let home_coords = vec![GpsHomeCoordinate {
            home_latitude: 40.7128,
            home_longitude: -74.0060,
            timestamp_us: 0,
        }];

        // Should return Ok even with empty GPS coordinates
        let result = export_to_gpx(
            &temp_input_path,
            0,
            1,
            &[],
            &home_coords,
            &export_opts,
            None,
            None,
        );
        assert!(
            result.is_ok(),
            "Export should succeed with empty GPS coordinates"
        );

        // Verify no GPX file is created when GPS coordinates are empty
        let gpx_path = temp_dir.path().join("test_input.gps.gpx");
        assert!(
            !gpx_path.exists(),
            "No GPX file should be created when GPS coordinates are empty"
        );

        Ok(())
    }

    #[test]
    fn test_gpx_trackpoints_skip_low_satellite_count() -> Result<()> {
        let home_coords = vec![];

        let gps_coords = vec![
            GpsCoordinate {
                latitude: 40.7129,
                longitude: -74.0061,
                altitude: 100.0,
                timestamp_us: 1_000_000,
                num_sats: Some(3), // Below minimum of 5
                speed: Some(5.0),
                ground_course: Some(180.0),
            },
            GpsCoordinate {
                latitude: 40.7130,
                longitude: -74.0062,
                altitude: 105.0,
                timestamp_us: 2_000_000,
                num_sats: Some(10), // Valid
                speed: Some(5.0),
                ground_course: Some(180.0),
            },
        ];

        let content = export_gpx_and_read(&gps_coords, &home_coords)?;

        // Verify that the trackpoint with low satellite count is not in output
        assert!(
            !content.contains("40.7129"),
            "Trackpoint with low satellite count should be excluded"
        );

        // Verify that the valid trackpoint is in output
        assert!(
            content.contains("40.7130"),
            "Trackpoint with sufficient satellites should be included"
        );

        Ok(())
    }

    #[test]
    fn test_firmware_prefix_for_revision() {
        assert_eq!(
            firmware_prefix_for_revision("EmuFlight 0.4.3"),
            Some("EMUF_")
        );
        assert_eq!(
            firmware_prefix_for_revision("EMUFLIGHT 0.4.3"),
            Some("EMUF_")
        );
        assert_eq!(
            firmware_prefix_for_revision("Betaflight 4.5.0"),
            Some("BTFL_")
        );
        assert_eq!(
            firmware_prefix_for_revision("Betaflight 2025.12.0-beta (abc) STM32H743"),
            Some("BTFL_")
        );
        assert_eq!(firmware_prefix_for_revision("INAV 7.1.2"), Some("INAV_"));
        assert_eq!(firmware_prefix_for_revision(""), None);
        assert_eq!(firmware_prefix_for_revision("Unknown firmware"), None);
    }

    #[test]
    fn test_corrected_session_base_name_matching_vendor() {
        let path = std::path::Path::new("/logs/EMUF_BLACKBOX_LOG_QUAD_20260531.BBL");
        assert_eq!(corrected_session_base_name(path, "EmuFlight 0.4.3"), None);

        let path = std::path::Path::new("/logs/BTFL_BLACKBOX_LOG_QUAD_20260531.BBL");
        assert_eq!(corrected_session_base_name(path, "Betaflight 4.5.0"), None);
    }

    #[test]
    fn test_corrected_session_base_name_mismatched_vendor() {
        let path = std::path::Path::new("/logs/EMUF_BLACKBOX_LOG_QUAD_20260531.BBL");
        assert_eq!(
            corrected_session_base_name(path, "Betaflight 2025.12.0-beta"),
            Some("BTFL_BLACKBOX_LOG_QUAD_20260531".to_string())
        );

        let path = std::path::Path::new("/logs/BTFL_BLACKBOX_LOG_QUAD_20260531.BBL");
        assert_eq!(
            corrected_session_base_name(path, "EmuFlight 0.4.3"),
            Some("EMUF_BLACKBOX_LOG_QUAD_20260531".to_string())
        );
    }

    #[test]
    fn test_corrected_session_base_name_unknown_prefix() {
        // BBL without a known prefix — no correction regardless of firmware
        let path = std::path::Path::new("/logs/BLACKBOX_LOG_QUAD_20260531.BBL");
        assert_eq!(corrected_session_base_name(path, "EmuFlight 0.4.3"), None);
        assert_eq!(corrected_session_base_name(path, "Betaflight 4.5.0"), None);
    }

    #[test]
    fn test_corrected_session_base_name_fork_exemption() {
        // Quicksilver (QUIC_) intentionally writes Betaflight revision headers — must NOT rename
        let path = std::path::Path::new("/logs/QUIC_Twiglet_2026-06-09_file_0.bfl");
        assert_eq!(
            corrected_session_base_name(path, "Betaflight 4.3.0"),
            None,
            "QUIC_ files with Betaflight revision must not be renamed to BTFL_"
        );
        // Sanity: an actual mismatch on a non-fork prefix still renames
        let path = std::path::Path::new("/logs/EMUF_BLACKBOX_LOG_20260531.BBL");
        assert_eq!(
            corrected_session_base_name(path, "Betaflight 4.3.0"),
            Some("BTFL_BLACKBOX_LOG_20260531".to_string())
        );
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_export_to_csv_reports_skip_reason() -> Result<()> {
        // Default BBLLog has zero duration and zero frames, so should_skip_export
        // takes the "no duration info" fallback path and skips on frame count.
        let log = BBLLog::new(1, 1);
        let temp_dir = TempDir::new()?;
        let input_path = temp_dir.path().join("test_input.bbl");

        let export_opts = ExportOptions {
            csv: true,
            gpx: false,
            event: false,
            output_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
            force_export: false,
        };

        let report = export_to_csv(&log, &input_path, &export_opts, None)?;

        assert!(report.csv_path.is_none(), "skipped log must not write CSV");
        let reason = report
            .skip_reason
            .expect("skipped log must carry a skip_reason");
        assert!(
            reason.contains("too few frames"),
            "expected frame-count skip reason, got: {reason}"
        );
        Ok(())
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_export_to_csv_no_skip_reason_when_forced() -> Result<()> {
        let log = BBLLog::new(1, 1);
        let temp_dir = TempDir::new()?;
        let input_path = temp_dir.path().join("test_input.bbl");

        let export_opts = ExportOptions {
            csv: true,
            gpx: false,
            event: false,
            output_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
            force_export: true,
        };

        let report = export_to_csv(&log, &input_path, &export_opts, None)?;

        assert!(
            report.skip_reason.is_none(),
            "forced export must not carry a skip_reason"
        );
        Ok(())
    }

    #[cfg(feature = "csv")]
    fn csv_test_log() -> BBLLog {
        use std::collections::HashMap;
        let mut log = BBLLog::new(2, 3);
        log.header.i_frame_def = FrameDefinition::from_field_names(vec![
            "loopIteration".into(),
            "time".into(),
            "vbatLatest".into(),
            "amperageLatest".into(),
            "gyroADC[0]".into(),
        ]);
        for (n, t) in [(0u32, 1000u64), (1, 2000), (2, 3000)] {
            let mut data: HashMap<String, i32> = HashMap::new();
            data.insert("loopIteration".into(), n as i32);
            data.insert("time".into(), t as i32);
            data.insert("vbatLatest".into(), 1600 + n as i32);
            data.insert("amperageLatest".into(), 250 * n as i32);
            data.insert("gyroADC[0]".into(), -7 * n as i32);
            log.frames.push(DecodedFrame {
                frame_type: if n == 0 { 'I' } else { 'P' },
                timestamp_us: t,
                loop_iteration: n,
                data,
            });
        }
        log
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_writer_output_is_byte_equal_to_file_output() -> Result<()> {
        let log = csv_test_log();
        let temp_dir = TempDir::new()?;
        let input_path = temp_dir.path().join("in.bbl");
        let export_opts = ExportOptions {
            csv: true,
            output_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
            force_export: true,
            ..ExportOptions::default()
        };

        let report = export_to_csv(&log, &input_path, &export_opts, None)?;
        let file_bytes = std::fs::read(report.csv_path.expect("csv written"))?;

        let mut mem: Vec<u8> = Vec::new();
        export_flight_data_to_writer(&log, &mut mem)?;

        assert!(!mem.is_empty());
        assert_eq!(mem, file_bytes);
        let text = String::from_utf8(mem)?;
        assert_eq!(text.lines().count(), 4, "header plus 3 data rows");
        assert!(text.starts_with("loopIteration, time (us), vbatLatest (V), amperageLatest (A)"));
        Ok(())
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_writer_writes_nothing_without_main_frames() -> Result<()> {
        let mut mem: Vec<u8> = Vec::new();
        export_flight_data_to_writer(&BBLLog::new(1, 1), &mut mem)?;
        assert!(mem.is_empty());
        Ok(())
    }

    #[cfg(feature = "csv")]
    fn csv_cells(text: &str) -> Vec<Vec<String>> {
        text.lines()
            .map(|line| line.split(',').map(|c| c.trim().to_string()).collect())
            .collect()
    }

    #[cfg(feature = "csv")]
    fn unit_test_log() -> BBLLog {
        let mut log = csv_test_log();
        log.header
            .all_headers
            .push("H Firmware type:Cleanflight".into());
        log.header
            .all_headers
            .push("H gyro_scale:0x3f800000".into());
        log.header.sysconfig.insert("acc_1G".into(), 2048);
        log
    }

    #[cfg(feature = "csv")]
    fn export_with(log: &BBLLog, units: &UnitOptions) -> Vec<Vec<String>> {
        let mut mem: Vec<u8> = Vec::new();
        export_flight_data_to_writer_with_units(log, &mut mem, units).unwrap();
        csv_cells(&String::from_utf8(mem).unwrap())
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_default_units_leave_output_unchanged() {
        let log = unit_test_log();
        let mut plain: Vec<u8> = Vec::new();
        export_flight_data_to_writer(&log, &mut plain).unwrap();
        let mut with_default: Vec<u8> = Vec::new();
        export_flight_data_to_writer_with_units(&log, &mut with_default, &UnitOptions::default())
            .unwrap();
        assert_eq!(plain, with_default);
        let text = String::from_utf8(plain).unwrap();
        assert!(text.starts_with(
            "loopIteration, time (us), vbatLatest (V), amperageLatest (A), gyroADC[0], energyCumulative (mAh)\n"
        ));
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_electrical_and_time_units() {
        let log = unit_test_log();
        let units = UnitOptions {
            vbat: VbatUnit::MilliVolts,
            amperage: AmperageUnit::MilliAmps,
            frame_time: FrameTimeUnit::Seconds,
            ..UnitOptions::default()
        };
        let rows = export_with(&log, &units);
        assert_eq!(
            rows[0],
            [
                "loopIteration",
                "time (s)",
                "vbatLatest (mV)",
                "amperageLatest (mA)",
                "gyroADC[0]",
                "energyCumulative (mAh)"
            ]
        );
        // raw vbat 1601 hundredths of a volt = 16010 mV; raw current 250 = 2500 mA
        assert_eq!(rows[2][..4], ["1", "0.002000", "16010", "2500"]);

        let units = UnitOptions {
            frame_time: FrameTimeUnit::Milliseconds,
            vbat: VbatUnit::Raw,
            amperage: AmperageUnit::Raw,
            ..UnitOptions::default()
        };
        let rows = export_with(&log, &units);
        assert_eq!(rows[0][1..4], ["time (ms)", "vbatLatest", "amperageLatest"]);
        assert_eq!(rows[3][..4], ["2", "3.000", "1602", "500"]);
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_rotation_units_from_header_scale() {
        let log = unit_test_log();
        // gyro_scale 1.0 deg/s per count: -14 counts = -14 deg/s = -14 * pi/180 rad/s
        let rows = export_with(
            &log,
            &UnitOptions {
                rotation: RotationUnit::DegreesPerSecond,
                ..UnitOptions::default()
            },
        );
        assert_eq!(rows[0][4], "gyroADC[0] (deg/s)");
        assert_eq!(rows[3][4], "-14.00");
        let rows = export_with(
            &log,
            &UnitOptions {
                rotation: RotationUnit::RadiansPerSecond,
                ..UnitOptions::default()
            },
        );
        assert_eq!(rows[0][4], "gyroADC[0] (rad/s)");
        assert_eq!(rows[3][4], "-0.24");
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_flags_raw_drops_suffix_and_names() {
        let mut log = unit_test_log();
        log.header.s_frame_def =
            FrameDefinition::from_field_names(vec!["time".into(), "flightModeFlags".into()]);
        log.frames[0].data.insert("flightModeFlags".into(), 3);
        let rows = export_with(&log, &UnitOptions::default());
        assert_eq!(rows[0].last().unwrap(), "flightModeFlags (flags)");
        assert_eq!(rows[1].last().unwrap(), "ANGLE_MODE|HORIZON_MODE");
        let rows = export_with(
            &log,
            &UnitOptions {
                flags: FlagsUnit::Raw,
                ..UnitOptions::default()
            },
        );
        assert_eq!(rows[0].last().unwrap(), "flightModeFlags");
        assert_eq!(rows[1].last().unwrap(), "3");
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_height_and_acceleration_units() {
        let mut log = unit_test_log();
        log.header.i_frame_def = FrameDefinition::from_field_names(vec![
            "loopIteration".into(),
            "time".into(),
            "BaroAlt".into(),
            "accSmooth[2]".into(),
        ]);
        for (i, frame) in log.frames.iter_mut().enumerate() {
            frame.data.insert("BaroAlt".into(), 1000 * (i as i32 + 1));
            frame
                .data
                .insert("accSmooth[2]".into(), 2048 * (i as i32 + 1));
        }
        // no unit given: BaroAlt column is left as it was
        let rows = export_with(&log, &UnitOptions::default());
        assert_eq!(rows[0][2..], ["BaroAlt", "accSmooth[2]"]);
        assert_eq!(rows[1][2..], ["1000", "2048"]);

        let units = UnitOptions {
            height: Some(HeightUnit::Feet),
            acceleration: AccelerationUnit::MetersPerSecondSquared,
            ..UnitOptions::default()
        };
        let rows = export_with(&log, &units);
        assert_eq!(rows[0][2..], ["BaroAlt (ft)", "accSmooth[2] (m/s/s)"]);
        // 1000 cm = 10 m = 32.8084 ft; 1 g = 9.80665 m/s^2
        assert_eq!(rows[1][2..], ["32.81", "9.81"]);
        assert_eq!(rows[3][2..], ["98.43", "29.42"]);

        let units = UnitOptions {
            height: Some(HeightUnit::Centimeters),
            acceleration: AccelerationUnit::Gs,
            ..UnitOptions::default()
        };
        let rows = export_with(&log, &units);
        assert_eq!(rows[0][2..], ["BaroAlt (cm)", "accSmooth[2] (g)"]);
        assert_eq!(rows[2][2..], ["2000", "2.00"]);
    }

    #[cfg(feature = "csv")]
    fn frame_with(frame_type: char, t: u64, fields: &[(&str, i32)]) -> DecodedFrame {
        DecodedFrame {
            frame_type,
            timestamp_us: t,
            loop_iteration: 0,
            data: fields.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_merge_gps_follows_blackbox_decode_row_rules() {
        let mut log = unit_test_log();
        log.header.g_frame_def = FrameDefinition::from_field_names(
            [
                "time",
                "GPS_numSat",
                "GPS_coord[0]",
                "GPS_coord[1]",
                "GPS_speed",
                "GPS_ground_course",
            ]
            .map(String::from)
            .to_vec(),
        );
        log.header.g_frame_def.update_predictors(&[
            PREDICT_LAST_MAIN_FRAME_TIME,
            0,
            PREDICT_HOME_COORD,
            PREDICT_HOME_COORD,
        ]);
        let main = log.frames.clone();
        let g1 = [
            ("time", 0),
            ("GPS_numSat", 7),
            ("GPS_coord[0]", 5_000_000),
            ("GPS_coord[1]", -1_000_000),
            ("GPS_speed", 1234),
            ("GPS_ground_course", 1234),
        ];
        let g2 = [
            ("time", 500),
            ("GPS_numSat", 8),
            ("GPS_coord[0]", 10_000_000),
            ("GPS_coord[1]", 0),
            ("GPS_speed", 100_000),
            ("GPS_ground_course", 0),
        ];
        log.frames = vec![
            // before any H frame: dropped
            frame_with('G', 900, &g2),
            frame_with(
                'H',
                900,
                &[("GPS_home[0]", 100_000_000), ("GPS_home[1]", -50_000_000)],
            ),
            main[0].clone(),
            frame_with('G', 1000, &g1), // offset 0: same time as the I frame
            main[1].clone(),
            frame_with('G', 2000, &g2), // offset 500: later than the P frame
            main[2].clone(),
        ];
        let units = UnitOptions {
            merge_gps: true,
            ..UnitOptions::default()
        };
        let rows = export_with(&log, &units);
        assert_eq!(
            rows[0][5..],
            [
                "energyCumulative (mAh)",
                "GPS_numSat",
                "GPS_coord[0]",
                "GPS_coord[1]",
                "GPS_speed (m/s)",
                "GPS_ground_course"
            ]
        );
        assert_eq!(rows.len(), 5);
        // I frame written with the G frame of its own time
        assert_eq!(rows[1][1], "1000".to_string());
        assert_eq!(
            rows[1][6..],
            ["7", "10.5000000", "-5.1000000", "12.34", "123.4"]
        );
        // P frame flushed with the old GPS data
        assert_eq!(rows[2][1], "2000");
        assert_eq!(rows[2][6], "7");
        // the G frame at 2500 repeats the P frame under its own time
        assert_eq!(rows[3][..2], ["1", "2500"]);
        assert_eq!(
            rows[3][6..],
            ["8", "11.0000000", "-5.0000000", "1000.00", "0.0"]
        );
        assert_eq!(rows[4][..2], ["2", "3000"]);
        assert_eq!(rows[4][6], "8");

        // kph and a log without GPS rows ahead of the first G frame
        let units = UnitOptions {
            merge_gps: true,
            gps_speed: GpsSpeedUnit::KilometersPerHour,
            ..UnitOptions::default()
        };
        let rows = export_with(&log, &units);
        assert_eq!(rows[0].last().unwrap(), "GPS_ground_course");
        assert_eq!(rows[0][9], "GPS_speed (km/h)");
        assert_eq!(rows[1][9], "44.42");
    }

    #[cfg(feature = "csv")]
    fn merge_gps_log(predictors: &[u8]) -> BBLLog {
        let mut log = unit_test_log();
        log.header.g_frame_def = FrameDefinition::from_field_names(
            ["time", "GPS_numSat", "GPS_coord[0]", "GPS_coord[1]"]
                .map(String::from)
                .to_vec(),
        );
        log.header.g_frame_def.update_predictors(predictors);
        log
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_merge_gps_honours_g_field_predictors() {
        let units = UnitOptions {
            merge_gps: true,
            ..UnitOptions::default()
        };
        let home = frame_with(
            'H',
            900,
            &[("GPS_home[0]", 100_000_000), ("GPS_home[1]", 0)],
        );
        let g = frame_with(
            'G',
            1000,
            &[
                ("time", 2500),
                ("GPS_numSat", 5),
                ("GPS_coord[0]", 120_000_000),
                ("GPS_coord[1]", 0),
            ],
        );
        // No predictors: time and coordinates are the logged values.
        let mut log = merge_gps_log(&[0, 0, 0, 0]);
        let main = log.frames.clone();
        log.frames = vec![home.clone(), main[0].clone(), g.clone()];
        let rows = export_with(&log, &units);
        assert_eq!(rows[2][1], "2500");
        assert_eq!(rows[2][7], "12.0000000");
        // Last-main-frame time and home predictors: offset and home are added.
        let mut log = merge_gps_log(&[PREDICT_LAST_MAIN_FRAME_TIME, 0, PREDICT_HOME_COORD, 0]);
        log.frames = vec![home, main[0].clone(), g];
        let rows = export_with(&log, &units);
        assert_eq!(rows[2][1], "3500");
        assert_eq!(rows[2][7], "22.0000000");
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_merge_gps_keeps_home_of_the_g_frame() {
        let units = UnitOptions {
            merge_gps: true,
            ..UnitOptions::default()
        };
        let mut log = merge_gps_log(&[PREDICT_LAST_MAIN_FRAME_TIME, 0, PREDICT_HOME_COORD, 0]);
        let main = log.frames.clone();
        let home = |lat: i32| frame_with('H', 900, &[("GPS_home[0]", lat), ("GPS_home[1]", 0)]);
        log.frames = vec![
            home(100_000_000),
            main[0].clone(),
            frame_with(
                'G',
                1000,
                &[
                    ("time", 0),
                    ("GPS_numSat", 5),
                    ("GPS_coord[0]", 5_000_000),
                    ("GPS_coord[1]", 0),
                ],
            ),
            home(200_000_000),
            main[1].clone(),
        ];
        let rows = export_with(&log, &units);
        // The second H frame must not move the position of the earlier G frame.
        assert_eq!(rows[1][7], "10.5000000");
        assert_eq!(rows[2][7], "10.5000000");
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_merge_gps_without_g_fields_is_plain_csv() {
        let log = unit_test_log();
        let merged = export_with(
            &log,
            &UnitOptions {
                merge_gps: true,
                ..UnitOptions::default()
            },
        );
        assert_eq!(merged, export_with(&log, &UnitOptions::default()));
    }

    #[cfg(feature = "csv")]
    #[test]
    fn test_raw_units_keep_stream_order_and_drop_suffixes() {
        let mut log = unit_test_log();
        log.frames.reverse();
        let sorted = export_with(&log, &UnitOptions::default());
        assert_eq!(sorted[1][1], "1000");
        let raw = export_with(&log, &UnitOptions::raw());
        assert_eq!(
            raw[0],
            [
                "loopIteration",
                "time",
                "vbatLatest",
                "amperageLatest",
                "gyroADC[0]",
                "energyCumulative (mAh)"
            ]
        );
        assert_eq!(raw[1][1], "3000");
        assert_eq!(raw[3][1], "1000");
        assert_eq!(raw[1][2], "1602");
    }

    #[test]
    fn test_gpx_alt_offset_is_added_to_elevation() -> Result<()> {
        let coords = vec![GpsCoordinate {
            latitude: 1.0,
            longitude: 2.0,
            altitude: 21.0,
            timestamp_us: 0,
            num_sats: Some(9),
            speed: None,
            ground_course: None,
        }];
        let temp_dir = TempDir::new()?;
        let input_path = temp_dir.path().join("in.bbl");
        let opts = ExportOptions {
            gpx: true,
            output_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
            ..ExportOptions::default()
        };
        let read = |offset: f64| -> Result<String> {
            let report = export_to_gpx_with_alt_offset(
                &input_path,
                0,
                1,
                &coords,
                &[],
                &opts,
                None,
                None,
                offset,
            )?;
            Ok(std::fs::read_to_string(report.gpx_path.unwrap())?)
        };
        assert!(read(0.0)?.contains("<ele>21.00</ele>"));
        assert!(read(-3.5)?.contains("<ele>17.50</ele>"));
        let plain = export_to_gpx(&input_path, 0, 1, &coords, &[], &opts, None, None)?;
        assert!(std::fs::read_to_string(plain.gpx_path.unwrap())?.contains("<ele>21.00</ele>"));
        Ok(())
    }
}
