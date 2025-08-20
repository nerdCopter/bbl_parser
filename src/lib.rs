//! BBL Parser Library
//!
//! A Rust library for parsing Betaflight/EmuFlight/INAV blackbox log files.
//! This library provides both in-memory data access and export capabilities.
//!
//! # Examples
//!
//! Basic usage:
//! ```rust,no_run
//! use bbl_parser::{parse_bbl_file, ExportOptions};
//! use std::path::Path;
//!
//! let export_options = ExportOptions::default();
//! let log = parse_bbl_file(Path::new("flight.BBL"), export_options, false).unwrap();
//! println!("Found {} frames", log.sample_frames.len());
//! ```

// Module declarations
mod bbl_format;
pub mod conversion;
pub mod error;
pub mod export;
pub mod parser;
pub mod types;

// Re-export everything from modules for convenience
#[allow(ambiguous_glob_reexports)]
pub use bbl_format::*;
#[allow(ambiguous_glob_reexports)]
pub use conversion::*;
#[allow(ambiguous_glob_reexports)]
pub use error::*;
#[allow(ambiguous_glob_reexports)]
pub use export::*;
#[allow(ambiguous_glob_reexports)]
pub use parser::*;
#[allow(ambiguous_glob_reexports)]
pub use types::*;

// Re-export Result type for convenience
pub use anyhow::Result;
