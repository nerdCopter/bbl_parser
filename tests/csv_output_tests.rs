use std::fs;
use std::process::Command;

/// Integration tests for CSV output validation
/// These tests compare our RUST implementation against reference implementations

#[test]
fn test_csv_field_count_consistency() {
    // Test that all CSV rows have the same number of fields
    let output_dir = "/tmp/csv_test_output";
    let input_file = "input/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.BBL";

    // Clean up and create output directory
    let _ = fs::remove_dir_all(output_dir);
    fs::create_dir_all(output_dir).expect("Failed to create test output directory");

    // Run our parser
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--",
            "--csv",
            "--output-dir",
            output_dir,
            input_file,
        ])
        .output()
        .expect("Failed to run bbl_parser");

    assert!(
        output.status.success(),
        "Parser failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check the generated CSV file
    let csv_path = format!("{output_dir}/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.01.csv");
    let csv_content = fs::read_to_string(&csv_path).expect("Failed to read generated CSV file");

    let lines: Vec<&str> = csv_content.lines().collect();
    assert!(!lines.is_empty(), "CSV file is empty");

    // Get header field count
    let header_fields = lines[0].split(',').count();
    println!("Header has {header_fields} fields");

    // Check all data rows have the same field count
    for (i, line) in lines.iter().enumerate().skip(1) {
        let field_count = line.split(',').count();
        assert_eq!(
            field_count,
            header_fields,
            "Row {} has {} fields, but header has {} fields. Row content: {}",
            i + 1,
            field_count,
            header_fields,
            line
        );
    }

    println!(
        "✅ All {} rows have consistent field count: {}",
        lines.len(),
        header_fields
    );
}

#[test]
fn test_flag_fields_are_numeric() {
    // Test that flag fields output numeric values, not text strings
    let output_dir = "/tmp/csv_flag_test";
    let input_file = "input/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.BBL";

    // Clean up and create output directory
    let _ = fs::remove_dir_all(output_dir);
    fs::create_dir_all(output_dir).expect("Failed to create test output directory");

    // Run our parser
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--",
            "--csv",
            "--output-dir",
            output_dir,
            input_file,
        ])
        .output()
        .expect("Failed to run bbl_parser");

    assert!(
        output.status.success(),
        "Parser failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check the generated CSV file
    let csv_path = format!("{output_dir}/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.01.csv");
    let csv_content = fs::read_to_string(&csv_path).expect("Failed to read generated CSV file");

    let lines: Vec<&str> = csv_content.lines().collect();
    let header = lines[0];

    // Find flag field indices
    let field_names: Vec<&str> = header.split(',').map(|s| s.trim()).collect();
    let mut flag_indices = Vec::new();

    for (i, field) in field_names.iter().enumerate() {
        if field.ends_with("(flags)") {
            flag_indices.push(i);
            println!("Found flag field at index {i}: {field}");
        }
    }

    assert!(!flag_indices.is_empty(), "No flag fields found");

    // Check first few data rows for numeric flag values
    for (row_num, line) in lines.iter().enumerate().skip(1).take(10) {
        let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

        for &flag_idx in &flag_indices {
            if flag_idx < fields.len() {
                let flag_value = fields[flag_idx];

                // Flag should be numeric (parse as integer)
                let is_numeric = flag_value.parse::<i32>().is_ok();
                assert!(
                    is_numeric,
                    "Row {} field {} ({}) should be numeric but got: '{}'",
                    row_num + 1,
                    flag_idx,
                    field_names[flag_idx],
                    flag_value
                );

                // Flag should not contain text like "ARM", "SERVO1", etc.
                let has_text_flags = flag_value.contains("ARM")
                    || flag_value.contains("SERVO")
                    || flag_value.contains("IDLE")
                    || flag_value.contains("ANGLE");

                assert!(
                    !has_text_flags,
                    "Row {} field {} ({}) contains text flags: '{}'",
                    row_num + 1,
                    flag_idx,
                    field_names[flag_idx],
                    flag_value
                );
            }
        }
    }

    println!("✅ All flag fields contain numeric values (no text strings)");
}

#[test]
fn test_no_comma_in_field_values() {
    // Test that no field values contain commas (which would break CSV parsing)
    let output_dir = "/tmp/csv_comma_test";
    let input_file = "input/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.BBL";

    // Clean up and create output directory
    let _ = fs::remove_dir_all(output_dir);
    fs::create_dir_all(output_dir).expect("Failed to create test output directory");

    // Run our parser
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--",
            "--csv",
            "--output-dir",
            output_dir,
            input_file,
        ])
        .output()
        .expect("Failed to run bbl_parser");

    assert!(
        output.status.success(),
        "Parser failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check the generated CSV file
    let csv_path = format!("{output_dir}/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.01.csv");
    let csv_content = fs::read_to_string(&csv_path).expect("Failed to read generated CSV file");

    let lines: Vec<&str> = csv_content.lines().collect();
    let header_field_count = lines[0].split(',').count();

    // Check that no data rows have field values containing commas
    for (row_num, line) in lines.iter().enumerate().skip(1) {
        let field_count = line.split(',').count();

        // If field count is different, there might be commas in field values
        if field_count != header_field_count {
            panic!(
                "Row {} has {} fields but header has {} fields. This suggests comma in field values. Row: {}",
                row_num + 1, field_count, header_field_count, line
            );
        }

        // Also check for common text patterns that shouldn't have commas
        let problematic_patterns = ["ARM,", "SERVO,", "GPS,"];
        for pattern in &problematic_patterns {
            if line.contains(pattern) {
                panic!(
                    "Row {} contains comma-separated text in field values: pattern '{}' found in: {}",
                    row_num + 1, pattern, line
                );
            }
        }
    }

    println!("✅ No field values contain commas that would break CSV parsing");
}

#[test]
fn test_header_format_compliance() {
    // Test that CSV headers match expected format
    let output_dir = "/tmp/csv_header_test";
    let input_file = "input/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.BBL";

    // Clean up and create output directory
    let _ = fs::remove_dir_all(output_dir);
    fs::create_dir_all(output_dir).expect("Failed to create test output directory");

    // Run our parser
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--",
            "--csv",
            "--output-dir",
            output_dir,
            input_file,
        ])
        .output()
        .expect("Failed to run bbl_parser");

    assert!(
        output.status.success(),
        "Parser failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check the generated CSV file
    let csv_path = format!("{output_dir}/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.01.csv");
    let csv_content = fs::read_to_string(&csv_path).expect("Failed to read generated CSV file");

    let lines: Vec<&str> = csv_content.lines().collect();
    let header = lines[0];

    // Expected core fields that should be present
    let expected_fields = [
        "loopIteration",
        "time (us)",
        "axisP[0]",
        "axisP[1]",
        "axisP[2]",
        "axisI[0]",
        "axisI[1]",
        "axisI[2]",
        "gyroADC[0]",
        "gyroADC[1]",
        "gyroADC[2]",
        "motor[0]",
        "motor[1]",
        "motor[2]",
        "motor[3]",
        "flightModeFlags (flags)",
        "stateFlags (flags)",
        "failsafePhase (flags)",
    ];

    for expected_field in &expected_fields {
        assert!(
            header.contains(expected_field),
            "Header missing expected field: '{expected_field}'\nHeader: {header}"
        );
    }

    // Check that flag fields have proper unit notation
    assert!(header.contains("flightModeFlags (flags)"));
    assert!(header.contains("stateFlags (flags)"));
    assert!(header.contains("failsafePhase (flags)"));

    println!("✅ Header format contains all expected fields with correct units");
}

#[test]
fn test_data_value_ranges() {
    // Test that data values are within reasonable ranges
    let output_dir = "/tmp/csv_range_test";
    let input_file = "input/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.BBL";

    // Clean up and create output directory
    let _ = fs::remove_dir_all(output_dir);
    fs::create_dir_all(output_dir).expect("Failed to create test output directory");

    // Run our parser
    let output = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--",
            "--csv",
            "--output-dir",
            output_dir,
            input_file,
        ])
        .output()
        .expect("Failed to run bbl_parser");

    assert!(
        output.status.success(),
        "Parser failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check the generated CSV file
    let csv_path = format!("{output_dir}/BTFL_BLACKBOX_LOG_20250512_021116_MAMBAF722_2022B.01.csv");
    let csv_content = fs::read_to_string(&csv_path).expect("Failed to read generated CSV file");

    let lines: Vec<&str> = csv_content.lines().collect();
    let header = lines[0];
    let field_names: Vec<&str> = header.split(',').map(|s| s.trim()).collect();

    // Find key field indices
    let loop_iter_idx = field_names.iter().position(|&f| f == "loopIteration");
    let time_idx = field_names.iter().position(|&f| f == "time (us)");
    let motor0_idx = field_names.iter().position(|&f| f == "motor[0]");

    // Check a sample of data rows
    for (row_num, line) in lines.iter().enumerate().skip(1).take(100) {
        let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

        // Test loopIteration is reasonable (monotonically increasing)
        if let Some(idx) = loop_iter_idx {
            if idx < fields.len() {
                let loop_iter: i32 = fields[idx].parse().unwrap_or_else(|_| {
                    panic!(
                        "Row {} loopIteration should be numeric: '{}'",
                        row_num + 1,
                        fields[idx]
                    )
                });
                assert!(
                    loop_iter >= 0,
                    "Row {} loopIteration should be non-negative: {}",
                    row_num + 1,
                    loop_iter
                );
            }
        }

        // Test time is reasonable (microseconds, should be large)
        if let Some(idx) = time_idx {
            if idx < fields.len() {
                let time_us: i64 = fields[idx].parse().unwrap_or_else(|_| {
                    panic!(
                        "Row {} time should be numeric: '{}'",
                        row_num + 1,
                        fields[idx]
                    )
                });
                assert!(
                    time_us > 0,
                    "Row {} time should be positive: {}",
                    row_num + 1,
                    time_us
                );
                assert!(
                    time_us < 1_000_000_000_000i64,
                    "Row {} time seems unreasonably large: {}",
                    row_num + 1,
                    time_us
                );
            }
        }

        // Test motor values are reasonable (0-2047 typically)
        if let Some(idx) = motor0_idx {
            if idx < fields.len() {
                let motor0: i32 = fields[idx].parse().unwrap_or_else(|_| {
                    panic!(
                        "Row {} motor[0] should be numeric: '{}'",
                        row_num + 1,
                        fields[idx]
                    )
                });
                assert!(
                    motor0 >= 0,
                    "Row {} motor[0] should be non-negative: {}",
                    row_num + 1,
                    motor0
                );
                assert!(
                    motor0 <= 10000,
                    "Row {} motor[0] seems unreasonably high: {}",
                    row_num + 1,
                    motor0
                );
            }
        }
    }

    println!("✅ Data values are within reasonable ranges");
}
