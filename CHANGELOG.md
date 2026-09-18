# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.1] - 2026-09-17

### Fixed
- **5-15s duration bucket gyro-activity check** (#66): `should_skip_export`'s 5-15s bucket only checked data density, never `has_minimal_gyro_activity` — a short, dense, but genuinely stationary log (bench test) passed through as kept purely on data rate, with zero movement verification. Merges the 5-15s and >=15s branches into one shared gyro-activity check after a single density guard clause. Closes IT #62
- **`vergen-gitcl` build-dependency gated behind `cli` feature** (#64): was an unconditional build-dependency requiring Rust 1.96.0 for every consumer regardless of selected library features, contradicting the declared MSRV. `build.rs` now skips the `Emitter` call when `cli` is disabled, so library-only consumers (`default-features = false`) never compile it
- **Release workflow asset upload permissions** (#61): `upload-release-assets` job had no `permissions` block, so `gh release upload`/`gh release edit` inherited the repo's read-only default `GITHUB_TOKEN` and failed with `HTTP 403`; scoped `contents: write` to that job only
- **`--force-export` CLI help text** (`src/main.rs`): described the pre-#66 filtering rule ("5-15s: kept if data density >1500fps" as if gyro activity were checked only above 15s); corrected to match the merged `should_skip_export` logic, where the gyro-activity check also applies to the 5-15s bucket after the density gate
- **`--event` CLI help text**: said "JSON files"; actual output is JSONL (one JSON object per line), matching the rest of the docs

### Changed
- **Dependency update** (#65): `Cargo.lock` refreshed to the latest versions already permitted by existing `Cargo.toml` semver requirements (anyhow 1.0.103→1.0.104, clap 4.6.1→4.6.7, glob 0.3.3→0.3.4, regex 1.12.4→1.13.1, serde 1.0.228→1.0.229, serde_json 1.0.150→1.0.151). No `Cargo.toml` requirement changes

### Documentation
- Corrected `README.md`/`OVERVIEW.md`: quick-start CLI examples used a nonexistent `--csv` flag — CSV export is always on for the CLI binary, and passing `--csv` fails with `error: unexpected argument '--csv' found`
- Corrected `examples/README.md`: CLI binary invocation examples (`bbl_parser flight.BBL ./output`) passed the output directory as a second positional argument; the CLI binary treats every positional argument as an input file pattern, unlike the example programs' own arg parsing — corrected to `--output-dir ./output`
- Corrected `README.md`/`OVERVIEW.md` smart-filtering bullet lists: wording implied the gyro-activity check applied only above 15s or was independent of the density gate; reworded to match the merged `should_skip_export` logic (5-15s bucket also gyro-checked after the density gate)
- Corrected `CRATE_USAGE.md` and `examples/README.md`: `export_to_gpx`/`export_to_event` code samples were missing the `log_start_datetime`/`base_name_override` parameters added since the functions' current signatures, and one GPX sample had an unbalanced brace that would not compile
- Corrected `CRATE_USAGE.md`: `BBLLog.gps_track` field name did not exist; actual field is `gps_coordinates`
- Corrected `OVERVIEW.md`/`GOALS.md`: stale "62 unit tests" count updated to the current 58
- Corrected `OVERVIEW.md`: project-structure diagram referenced a nonexistent `bbl_format.rs` and omitted `filters.rs`
- Corrected `AGENTS.md`: test-location list omitted `src/export.rs` and `src/filters.rs`
- Corrected `examples/event_export.rs`: doc comment claimed the parser returns empty event vectors; event parsing has populated `event_frames` since commit 40f9311
- Corrected `OVERVIEW.md`/`examples/README.md` sample CLI output: frame-type counts (I/P/S/G/H/E) summed to 85005, not the stated "Total frames: 84235"
- Corrected `examples/README.md` sample event list: showed 2 of 4 events with a false "... and 2 more events" line — the real code (`.take(5)`) prints all events when the total is ≤5
- Corrected `GOALS.md`: "gyro variance heuristics" is stale terminology from before `calculate_variance` was deprecated in favor of `calculate_range`; updated to "gyro range/activity heuristics"

## [1.1.0] - 2026-09-17

### Added
- **`ExportReport.skip_reason: Option<String>`**: `export_to_csv` now reports why a flight was filtered out (too short, low data density, minimal gyro activity, too few frames), instead of discarding the reason and returning only `csv_path: None` (#59)
- **`-O`/`-F` short flags**: `-O` for `--output-dir`, `-F` for `--force-export`; also adds a hidden `--force` long alias for `--force-export` (#52)

### Fixed
- **Library filtering parity** (#57): `export_to_csv` now applies `should_skip_export` filtering directly, so `ExportOptions.force_export` has effect for library consumers calling `export_to_csv` without going through the CLI. Previously the heuristic only ran in the CLI's `main()`
- **Frame-parsing progress print**: the unconditional 100,000-frame progress print in `parse_bbl_file_all_logs`/`parse_bbl_bytes_all_logs` is now fully gated behind the `debug` flag, matching the existing 50,000-frame branch (#57)
- **Case-insensitive glob matching** (#54): `expand_input_paths_with_depth()` used the `glob` crate's default case-sensitive matching, contradicting documented case-insensitive behavior; switched to `glob_with()` with `case_sensitive: false`
- **Release workflow `--repo`/`--clobber` placement** (#48): `find -exec gh release upload ... \; --repo ... --clobber` passed `--repo`/`--clobber` as `find` predicates instead of `gh` arguments, failing release asset uploads; moved `\;` to the end of the `-exec` invocation

### Changed
- **Semver note**: `ExportReport` gained a new public field. Treated as a minor addition — the struct is return-only, with no known consumer constructing it via struct literal or exhaustive pattern match.

## [1.0.1] - 2026-07-02

### Added
- **Firmware vendor transition detection**: detects when a BBL/BFL file spans multiple firmware vendors (e.g. Betaflight sessions recorded before a board was reflashed to EmuFlight), warns with per-vendor session ranges, and corrects per-session output filename prefixes accordingly (prevents duplicate/incorrect prefixes like `BTFL_BTFL_*` or `EMUF_BTFL_*`)
- **QUIC_ fork exemption**: `FORK_REVISION_MAP` exempts known Betaflight forks (Quicksilver/`QUIC_`) from false-positive vendor-transition renames, since Quicksilver intentionally writes Betaflight revision headers for Blackbox Explorer compatibility
- **`sanitize_base_name_override()`**: prevents path traversal via the public library API's base-name override
- **`vendor_name_for_prefix()`**: single source of truth for prefix-to-display-name mapping

### Fixed
- **Universal gyro activity filtering** (#40): ground-test logs without duration metadata (common in INAV and older Betaflight) no longer produce "Data Unavailable" output. Replaced scale-dependent variance detection with scale-independent gyro range detection (`MIN_GYRO_RANGE = 500.0`), lowered the no-duration frame fallback threshold (`FALLBACK_MIN_FRAMES = 7,500`), and fixed NaN propagation in `calculate_range()` for conservative handling of bad data. `calculate_variance()` is retained but deprecated (`#[deprecated(since = "1.0.0")]`) in favor of `calculate_range()`.
- **Release workflow**: corrected the binary version check that compared the full `bbl_parser 1.0.0 <sha> (<date>)` output against a string missing the `bbl_parser` prefix
- **`time` crate RUSTSEC-2026-0009**: updated the transitive `time` dependency (via the `vergen`/`vergen-gitcl` build-dependency) to 0.3.53, patching a denial-of-service via stack exhaustion (CVSS 6.8, medium)

### Changed
- **Release artifact packaging**: CI and release workflows now build ZIP archives (`bbl_parser`/`bbl_parser.exe` inside) instead of uploading loose binaries, for cleaner, versioned release pages
- **Dependency update**: broad `cargo update` sweep (clap 4.5.40 → 4.6.1, serde 1.0.140 → 1.0.150, regex 1.11.1 → 1.12.4, syn 2.0.103 → 2.0.118, libc 0.2.173 → 0.2.186, and others)
- **Build tooling**: migrated `vergen` 8 → `vergen-gitcl` 10 (`EmitBuilder`/`git_sha`/`git_commit_date` → `Emitter`/`Gitcl::all_git()`); `VERGEN_GIT_SHA`/`VERGEN_GIT_COMMIT_DATE` env var names are unchanged, so no downstream code changes were required

## [1.0.0] - 2025-12-29

### Added
- **Complete BBL binary format parser** with support for all frame types (I, P, S, H, G, E)
- **Library API** (`bbl_parser` crate) for programmatic access to parsing and export functions
- **CSV export** with blackbox_decode-compatible field ordering and formatting
- **GPX export** for GPS track visualization and mapping applications
- **Event export** in JSONL format with official Betaflight FlightLogEvent enum mapping
- **Multi-log support** with automatic detection and separate file generation
- **Smart export filtering** based on flight duration and gyro activity heuristics
- **Streaming architecture** for memory-efficient processing of large files (tested: 375K+ frames)
- **Comprehensive CLI** with configurable output options and batch processing
- **Feature flags** (`csv`, `json`, `cli`, `serde`) for flexible dependency management
- **Firmware compatibility** for Betaflight (4.5+), EmuFlight, and INAV
- **Unit conversions** for voltage (raw to volts) and current (raw to amps)
- **Energy calculation** with cumulative amperage integration
- **Betaflight-accurate flag formatting** for flight mode, state, and failsafe phases
- **GPS coordinate conversion** from NE (north-east) to standard GPS coordinates
- **Comprehensive test coverage** with 62 unit tests + 8 integration tests
- **Complete documentation** including README, CRATE_USAGE.md, OVERVIEW.md, and 8 examples
- **API documentation** with rustdoc comments on all public types and functions
- **GitHub Actions CI/CD** with multi-platform testing (Linux, Windows, macOS)
- **Automated release workflow** for crates.io publication with artifact management

### Technical Highlights
- **Pure Rust implementation** with no external binary dependencies
- **Production-grade error handling** using `anyhow::Result<T>` throughout public API
- **Type-safe design** with minimal unsafe code
- **Cross-platform support** verified on Ubuntu, Windows, and macOS
- **Dual licensing:** AGPL-3.0-or-later (open source) + commercial option available

### Performance
- Efficiently processes large blackbox logs via streaming architecture
- Tested on files up to 375K+ frames (21 MB) in under 7 seconds
- Memory-efficient frame processing with selective storage for analysis

### Compatibility
- **Input formats:** .BBL, .BFL, .TXT (case-insensitive)
- **Firmware versions:** Betaflight 4.0+, EmuFlight, INAV
- **Output formats:** CSV, GPX, JSONL event logs
- **Binary compatibility:** Output matches blackbox_decode reference implementation

### Documentation
- Extensive project documentation (README.md, OVERVIEW.md, CRATE_USAGE.md)
- Frame format specifications in FRAMES.md
- Usage examples for CSV, GPX, and event exports
- Contribution guidelines and commercial licensing information

---

## Future Roadmap

### Planned for 1.x series
- IMU angle computation (roll, pitch, yaw) from gyro/accelerometer/magnetometer data
- Extended unit conversions (altitude, speed, rotation rates, acceleration)
- GPS data integration into main CSV output
- Enhanced loop timing statistics and frame distribution analysis
- Parallel frame processing for multi-log files
- Advanced filtering options for specialized analysis
- Raw mode output (unprocessed sensor values)
- Current meter simulation improvements
- Extended firmware version testing coverage

---

## Known Limitations & Future Work

The following are not blocking 1.0.0 but may be addressed in future releases:

- IMU simulation features not yet implemented
- Some unit conversion types not yet available (altitude, speed, rotation rates)
- GPS data is exported separately (.gps.gpx) rather than integrated into CSV
- Raw sensor value export mode not available
- Some `.unwrap()` calls in test/example code (critical paths use proper Result handling)

---

## Acknowledgments

This project is built on extensive analysis of:
- [Betaflight blackbox-log-viewer](https://github.com/betaflight/blackbox-log-viewer)
- [Betaflight blackbox-tools](https://github.com/betaflight/blackbox-tools)

The implementation ensures compatibility with established blackbox format specifications
while providing the benefits of a modern, type-safe Rust library.

---

## Version History

- **1.1.1** (2026-09-17) - 5-15s gyro-activity filter fix, vergen-gitcl MSRV gating, release workflow permissions fix, doc accuracy corrections
- **1.1.0** (2026-09-17) - Export skip-reason reporting, library filtering parity, case-insensitive glob matching, `-O`/`-F` short flags
- **1.0.1** (2026-07-02) - Firmware vendor transition detection, universal gyro activity filtering fix, dependency updates
- **1.0.0** (2025-12-29) - First stable release
- **0.9.0** (2025-08+) - Development releases leading up to 1.0.0
