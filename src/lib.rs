//! BBL Parser Library
//!
//! A Rust library for parsing Betaflight/EmuFlight/INAV blackbox log files.
//! This library provides both in-memory data access and export capabilities.
//!
//! # Features
//!
//! - **`csv`** (default): Enable CSV export functionality
//! - **`cli`** (default): Build the command-line interface binary
//! - **`json`**: Enable event export in JSON format
//! - **`serde`**: Enable serialization/deserialization of types
//!
//! # Quick Start
//!
//! Parse a log file and access frame data:
//! ```rust,no_run
//! use bbl_parser::{parse_bbl_file, ExportOptions};
//! use std::path::Path;
//!
//! let export_options = ExportOptions::default();
//! let log = parse_bbl_file(Path::new("flight.BBL"), export_options, false).unwrap();
//! println!("Parsed {} frames", log.frames.len());
//! println!("Flight duration: {} μs", log.stats.end_time_us - log.stats.start_time_us);
//! ```
//!
//! Export to CSV format:
//! ```rust,no_run
//! use bbl_parser::{parse_bbl_file, ExportOptions, export_to_csv};
//! use std::path::Path;
//!
//! let export_options = ExportOptions {
//!     csv: true,
//!     gpx: false,
//!     event: false,
//!     output_dir: None,
//!     force_export: false,
//! };
//! let log = parse_bbl_file(Path::new("flight.BBL"), export_options.clone(), false).unwrap();
//! let report = export_to_csv(&log, Path::new("flight.BBL"), &export_options).unwrap();
//! if let Some(path) = report.csv_path {
//!     println!("Exported to: {}", path.display());
//! }
//! ```
//!
//! # Public API
//!
//! ## Parsing Functions
//! - [`parse_bbl_file`] - Parse a BBL file and return the first log
//! - [`parse_bbl_file_all_logs`] - Parse a BBL file and return all logs
//! - [`parse_bbl_bytes`] - Parse BBL data from memory
//! - [`parse_bbl_bytes_all_logs`] - Parse multiple logs from memory
//! - [`parse_single_log`] - Low-level API for streaming scenarios
//!
//! ## Data Types
//! - [`BBLLog`] - Complete parsed log with all frames and metadata
//! - [`ExportOptions`] - Configuration for export operations
//! - [`ExportReport`] - Results of export operations with output paths
//! - [`DecodedFrame`] - Individual frame with parsed data
//! - [`FrameDefinition`] - Frame structure metadata
//!
//! ## Export Functions
//! - [`export_to_csv`] - Export flight data to CSV format
//! - [`export_to_gpx`] - Export GPS data to GPX format
//! - [`export_to_event`] - Export event data to JSON format
//! - [`compute_export_paths`] - Helper for consistent path computation
//!
//! ## Filtering Functions
//! - [`should_skip_export`] - Determine if log should be skipped based on heuristics
//! - [`has_minimal_gyro_activity`] - Detect ground tests vs actual flights
//! - [`calculate_variance`] - Statistical helper for gyro analysis
//!
//! ## Conversion Utilities
//! - [`convert_amperage_to_amps`] - Convert raw amperage to amps
//! - [`convert_vbat_to_volts`] - Convert raw voltage to volts
//! - [`format_flight_mode_flags`] - Format flight mode as human-readable text
//! - [`format_state_flags`] - Format state flags as human-readable text
//! - [`format_failsafe_phase`] - Format failsafe phase as text

// Module declarations
pub mod conversion;
pub mod error;
pub mod export;
pub mod filters;
pub mod parser;
pub mod types;

// Re-export everything from modules for convenience
// This maintains backward compatibility while keeping the implementation flexible
#[allow(ambiguous_glob_reexports)]
pub use conversion::*;
#[allow(ambiguous_glob_reexports)]
pub use error::*;
#[allow(ambiguous_glob_reexports)]
pub use export::*;
#[allow(ambiguous_glob_reexports)]
pub use filters::*;
#[allow(ambiguous_glob_reexports)]
pub use parser::*;
#[allow(ambiguous_glob_reexports)]
pub use types::*;

// Re-export Result type for convenience
pub use anyhow::Result;
