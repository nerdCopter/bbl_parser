# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- **Firmware versions:** Betaflight 4.5+, EmuFlight, INAV
- **Output formats:** CSV, GPX, JSONL event logs
- **Binary compatibility:** Output matches blackbox_decode reference implementation

### Documentation
- Extensive project documentation (README.md, OVERVIEW.md, CRATE_USAGE.md)
- Frame format specifications in FRAMES.md
- Usage examples for CSV, GPX, and event exports
- Contribution guidelines and commercial licensing information

---

## Future Roadmap

### Planned for 1.1.0
- IMU angle computation (roll, pitch, yaw) from gyro/accelerometer/magnetometer data
- Extended unit conversions (altitude, speed, rotation rates, acceleration)
- GPS data integration into main CSV output
- Enhanced loop timing statistics and frame distribution analysis
- Parallel frame processing for multi-log files

### Planned for 1.x series
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

- **1.0.0** (2025-12-29) - First stable release
- **0.9.0** (2025-08+) - Development releases leading up to 1.0.0
