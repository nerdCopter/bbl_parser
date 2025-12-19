//! Integration tests for export functionality
//!
//! Tests the export layer across different scenarios:
//! - GPX export with directory creation
//! - Event export with JSON format
//! - Multi-log suffix handling (.NN)
//! - Output directory defaulting to input parent
//! - Error handling for edge cases

use bbl_parser::export::*;
use bbl_parser::{EventFrame, ExportOptions, GpsCoordinate};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_export_gpx_creates_output_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let nonexistent_dir = temp_dir.path().join("nonexistent").join("output");

    let bbl_path = temp_dir.path().join("test.bbl");
    let gps_coords = vec![GpsCoordinate {
        latitude: 40.7129,
        longitude: -74.0061,
        altitude: 100.0,
        timestamp_us: 54311755,
        num_sats: Some(10),
        speed: Some(5.0),
        ground_course: Some(180.0),
    }];

    let export_opts = ExportOptions {
        csv: false,
        gpx: true,
        event: false,
        output_dir: Some(nonexistent_dir.to_str().unwrap().to_string()),
        force_export: false,
    };

    let result = export_to_gpx(&bbl_path, 0, 1, &gps_coords, &[], &export_opts, None);
    assert!(
        result.is_ok(),
        "GPX export should succeed and create directories"
    );

    // Verify output directory was created
    assert!(
        nonexistent_dir.exists(),
        "Output directory should be created"
    );

    // Verify GPX file was created
    let gpx_path = nonexistent_dir.join("test.gps.gpx");
    assert!(
        gpx_path.exists(),
        "GPX file should be created in new directory"
    );
}

#[test]
fn test_export_event_creates_output_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let nonexistent_dir = temp_dir.path().join("event_out");

    let bbl_path = temp_dir.path().join("test.bbl");
    let event_frames = vec![
        EventFrame {
            event_name: "Disarm".to_string(),
            timestamp_us: 143932686,
            event_type: 13, // EVT_END
            event_data: Vec::new(),
        },
        EventFrame {
            event_name: "Flight mode change".to_string(),
            timestamp_us: 143905899,
            event_type: 8, // EVT_MODE
            event_data: Vec::new(),
        },
    ];

    let export_opts = ExportOptions {
        csv: false,
        gpx: false,
        event: true,
        output_dir: Some(nonexistent_dir.to_str().unwrap().to_string()),
        force_export: false,
    };

    let result = export_to_event(&bbl_path, 0, 1, &event_frames, &export_opts);
    assert!(
        result.is_ok(),
        "Event export should succeed and create directory"
    );

    // Verify directory and file created
    assert!(
        nonexistent_dir.exists(),
        "Event output directory should be created"
    );
    let event_path = nonexistent_dir.join("test.event");
    assert!(event_path.exists(), "Event file should be created");

    // Verify event content
    let content = fs::read_to_string(&event_path).expect("Failed to read event file");
    assert!(
        content.contains("Disarm"),
        "Event file should contain Disarm event"
    );
    assert!(
        content.contains("Flight mode change"),
        "Event file should contain Flight mode change"
    );
}

#[test]
fn test_export_event_empty_returns_ok() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let bbl_path = temp_dir.path().join("test.bbl");

    let export_opts = ExportOptions {
        csv: false,
        gpx: false,
        event: true,
        output_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
        force_export: false,
    };

    let result = export_to_event(&bbl_path, 0, 1, &[], &export_opts);
    assert!(
        result.is_ok(),
        "Event export should succeed with empty events"
    );

    // Verify no event file created
    let event_path = temp_dir.path().join("test.event");
    assert!(
        !event_path.exists(),
        "No event file should be created for empty events"
    );
}

#[test]
fn test_compute_export_paths_single_log() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let input_path = temp_dir.path().join("test.bbl");
    let output_dir = temp_dir.path().join("output");

    let export_opts = ExportOptions {
        csv: true,
        gpx: true,
        event: true,
        output_dir: Some(output_dir.to_str().unwrap().to_string()),
        force_export: false,
    };

    let (csv_path, _headers_path, gpx_path, event_path) =
        compute_export_paths(&input_path, &export_opts, 1, 1);

    // Verify no .NN suffix for single log
    assert!(
        csv_path.to_string_lossy().ends_with("test.csv"),
        "CSV path should not have .NN suffix for single log"
    );
    assert!(
        gpx_path.to_string_lossy().ends_with("test.gps.gpx"),
        "GPX path should be correct for single log"
    );
    assert!(
        event_path.to_string_lossy().ends_with("test.event"),
        "Event path should be correct for single log"
    );
}

#[test]
fn test_compute_export_paths_multi_log() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let input_path = temp_dir.path().join("test.bbl");
    let output_dir = temp_dir.path().join("output");

    let export_opts = ExportOptions {
        csv: true,
        gpx: true,
        event: true,
        output_dir: Some(output_dir.to_str().unwrap().to_string()),
        force_export: false,
    };

    let (csv_path, _headers_path, gpx_path, event_path) =
        compute_export_paths(&input_path, &export_opts, 2, 3);

    // Verify .NN suffix is applied for multi-log
    assert!(
        csv_path.to_string_lossy().contains("test.02.csv"),
        "CSV path should have .02 suffix for second log of three"
    );
    assert!(
        gpx_path.to_string_lossy().contains("test.02.gps.gpx"),
        "GPX path should have .02 suffix"
    );
    assert!(
        event_path.to_string_lossy().contains("test.02.event"),
        "Event path should have .02 suffix"
    );
}

#[test]
fn test_export_options_defaults() {
    let opts = ExportOptions::default();
    assert!(!opts.csv, "Default CSV should be false");
    assert!(!opts.gpx, "Default GPX should be false");
    assert!(!opts.event, "Default event should be false");
    assert!(
        opts.output_dir.is_none(),
        "Default output_dir should be None"
    );
    assert!(!opts.force_export, "Default force_export should be false");
}

#[test]
fn test_export_options_custom() {
    let opts = ExportOptions {
        csv: true,
        gpx: true,
        event: false,
        output_dir: Some("/tmp/test".to_string()),
        force_export: true,
    };

    assert!(opts.csv);
    assert!(opts.gpx);
    assert!(!opts.event);
    assert_eq!(opts.output_dir.as_ref().unwrap(), "/tmp/test");
    assert!(opts.force_export);
}

#[test]
fn test_gpx_empty_coordinates_returns_ok() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let bbl_path = temp_dir.path().join("test.bbl");

    let export_opts = ExportOptions {
        csv: false,
        gpx: true,
        event: false,
        output_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
        force_export: false,
    };

    // Should return Ok even with empty GPS coordinates
    let result = export_to_gpx(&bbl_path, 0, 1, &[], &[], &export_opts, None);
    assert!(
        result.is_ok(),
        "Export should succeed with empty GPS coordinates"
    );

    // Verify no GPX file is created when GPS coordinates are empty
    let gpx_path = temp_dir.path().join("test.gps.gpx");
    assert!(
        !gpx_path.exists(),
        "No GPX file should be created when GPS coordinates are empty"
    );
}
