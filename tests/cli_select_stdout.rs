//! CLI tests for `--index`, `--stdout` and `--limits`.
//!
//! The fixture is a synthetic multi-log BBL file: each log holds I frames only,
//! and its `gyroADC[0]` values encode the log number, so the log that was
//! decoded can be identified from the output.
#![cfg(feature = "cli")]

use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

const MARKER: &str = "H Product:Blackbox flight data recorder by Nicholas Sherlock\n";

fn unsigned_vb(mut v: u32, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn signed_vb(v: i32, out: &mut Vec<u8>) {
    unsigned_vb(((v << 1) ^ (v >> 31)) as u32, out);
}

/// One log: gyro values are `log_number * 100 - 3 + iteration` for 6 frames.
fn synthetic_log(log_number: u32) -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(MARKER.as_bytes());
    for line in [
        "H Data version:2",
        "H Firmware revision:Betaflight 4.4.0 (abc1234) TESTBOARD",
        "H looptime:500",
        "H Field I name:loopIteration,time,gyroADC[0]",
        "H Field I signed:0,0,1",
        "H Field I predictor:0,0,0",
        "H Field I encoding:1,1,0",
    ] {
        d.extend_from_slice(line.as_bytes());
        d.push(b'\n');
    }
    for it in 0..6u32 {
        d.push(b'I');
        unsigned_vb(it, &mut d);
        unsigned_vb(1_000_000 + it * 1000, &mut d);
        signed_vb(log_number as i32 * 100 - 3 + it as i32, &mut d);
    }
    d
}

fn write_fixture(dir: &Path, logs: u32) -> std::path::PathBuf {
    let mut data = Vec::new();
    for n in 1..=logs {
        data.extend(synthetic_log(n));
    }
    let path = dir.join("fixture.bbl");
    std::fs::write(&path, data).unwrap();
    path
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bbl_parser"))
        .args(args)
        .output()
        .unwrap()
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

#[test]
fn index_exports_only_the_chosen_log_with_upstream_suffix() {
    let tmp = TempDir::new().unwrap();
    let f = write_fixture(tmp.path(), 3);
    let out = run(&["-F", "--index", "2", f.to_str().unwrap()]);
    assert!(out.status.success(), "{}", s(&out.stderr));

    let csv = tmp.path().join("fixture.02.csv");
    assert!(csv.exists(), "log 2 must keep its .02 suffix");
    assert!(!tmp.path().join("fixture.01.csv").exists());
    assert!(!tmp.path().join("fixture.03.csv").exists());

    let body = std::fs::read_to_string(csv).unwrap();
    let rows: Vec<&str> = body.lines().skip(1).collect();
    assert_eq!(rows.len(), 6);
    let first: Vec<&str> = rows[0].split(',').map(str::trim).collect();
    assert_eq!(first[2], "197", "log 2 gyro starts at 2*100-3");
}

#[test]
fn index_out_of_range_reports_upstream_message_and_writes_nothing() {
    let tmp = TempDir::new().unwrap();
    let f = write_fixture(tmp.path(), 3);
    for bad in ["4", "999999"] {
        let out = run(&["-F", "--index", bad, f.to_str().unwrap()]);
        assert!(!out.status.success());
        let err = s(&out.stderr);
        assert!(
            err.contains(&format!(
                "Couldn't load log #{bad} from this file, because there are only 3 logs in total."
            )),
            "{err}"
        );
    }
    assert!(!tmp.path().join("fixture.01.csv").exists());
}

#[test]
fn index_zero_negative_and_non_numeric_are_rejected() {
    let tmp = TempDir::new().unwrap();
    let f = write_fixture(tmp.path(), 2);
    for bad in ["0", "-1", "abc", "1.5", "99999999999999999999999"] {
        let out = run(&["-F", "--index", bad, f.to_str().unwrap()]);
        assert!(!out.status.success(), "--index {bad} must fail");
    }
    assert!(!tmp.path().join("fixture.01.csv").exists());
}

#[test]
fn stdout_csv_is_byte_equal_to_file_csv_and_stdout_is_pure_csv() {
    let tmp = TempDir::new().unwrap();
    let f = write_fixture(tmp.path(), 3);

    let file_run = run(&["-F", "--index", "3", f.to_str().unwrap()]);
    assert!(file_run.status.success());
    let file_csv = std::fs::read(tmp.path().join("fixture.03.csv")).unwrap();

    let out = run(&["-F", "--index", "3", "--stdout", f.to_str().unwrap()]);
    assert!(out.status.success(), "{}", s(&out.stderr));
    assert_eq!(out.stdout, file_csv);

    let text = s(&out.stdout);
    assert!(text.starts_with("loopIteration, time (us), gyroADC[0]\n"));
    assert!(!text.contains("bbl_parser"), "banner must not reach stdout");
    assert!(!text.contains("Processing:"));
    let err = s(&out.stderr);
    assert!(err.contains("bbl_parser"), "version goes to stderr: {err}");
    assert!(err.contains("Processing: fixture.bbl"));

    // stdout mode writes no files besides the fixture and the earlier file run
    let mut names: Vec<String> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["fixture.03.csv", "fixture.03.headers.csv", "fixture.bbl"]
    );
}

#[test]
fn stdout_single_log_file_needs_no_index() {
    let tmp = TempDir::new().unwrap();
    let f = write_fixture(tmp.path(), 1);
    let out = run(&["-F", "--stdout", f.to_str().unwrap()]);
    assert!(out.status.success(), "{}", s(&out.stderr));
    assert_eq!(s(&out.stdout).lines().count(), 7);
}

#[test]
fn stdout_multi_log_without_index_lists_logs_and_emits_no_csv() {
    let tmp = TempDir::new().unwrap();
    let f = write_fixture(tmp.path(), 3);
    let out = run(&["-F", "--stdout", f.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty(), "no CSV bytes on failure");
    let err = s(&out.stderr);
    assert!(err.contains("please choose one with the --index argument"));
    assert!(err.contains("Index  Start offset  Size (bytes)"));
    assert!(err.contains("    1             0 "), "{err}");
}

#[test]
fn stdout_rejects_conflicting_flags_and_multiple_files() {
    let tmp = TempDir::new().unwrap();
    let a = write_fixture(tmp.path(), 1);
    let b = tmp.path().join("second.bbl");
    std::fs::copy(&a, &b).unwrap();
    let (a, b) = (a.to_str().unwrap(), b.to_str().unwrap());

    for extra in ["--debug", "--gpx", "--gps", "--event"] {
        let out = run(&["-F", "--stdout", extra, a]);
        assert!(!out.status.success(), "--stdout with {extra} must fail");
        assert!(out.stdout.is_empty());
    }
    let out = run(&["-F", "--stdout", "-O", "x", a]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());

    let out = run(&["-F", "--stdout", a, b]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(s(&out.stderr).contains("only decode one log at a time"));
}

#[test]
fn limits_prints_table_with_exact_min_max_range() {
    let tmp = TempDir::new().unwrap();
    let f = write_fixture(tmp.path(), 2);
    let out = run(&["-F", "--index", "2", "--limits", f.to_str().unwrap()]);
    assert!(out.status.success(), "{}", s(&out.stderr));
    let text = s(&out.stdout);
    assert!(text.contains("    Field name          Min          Max        Range"));
    // iteration 0..5, time 1000000..1005000, gyro 197..202
    assert!(text.contains(" loopIteration            0            5            5\n"));
    assert!(text.contains("          time      1000000      1005000         5000\n"));
    assert!(text.contains("    gyroADC[0]          197          202            5\n"));
}

#[test]
fn limits_with_stdout_keeps_stdout_pure_csv() {
    let tmp = TempDir::new().unwrap();
    let f = write_fixture(tmp.path(), 1);
    let out = run(&["-F", "--stdout", "--limits", f.to_str().unwrap()]);
    assert!(out.status.success());
    assert!(!s(&out.stdout).contains("Field name"));
    assert!(s(&out.stderr).contains("Field name"));
    assert_eq!(s(&out.stdout).lines().count(), 7);
}

#[test]
fn without_new_flags_every_log_is_exported() {
    let tmp = TempDir::new().unwrap();
    let f = write_fixture(tmp.path(), 3);
    let out = run(&["-F", f.to_str().unwrap()]);
    assert!(out.status.success());
    for n in 1..=3 {
        assert!(tmp.path().join(format!("fixture.0{n}.csv")).exists());
    }
}
