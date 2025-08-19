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
pub mod types;
pub mod parser;
pub mod conversion;
pub mod export;
pub mod error;

// Re-export everything from modules for convenience
pub use types::*;
pub use parser::*;
pub use conversion::*;
pub use export::*;
pub use error::*;
pub use bbl_format::*;

// Re-export Result type for convenience
pub use anyhow::Result;
