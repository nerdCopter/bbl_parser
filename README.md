# BBL Parser

A fast, pure-Rust Blackbox Log parser primarily used as a command-line tool, with an optional Rust crate API.

Supports `.BBL`, `.BFL`, `.TXT` (case-insensitive) across Betaflight, EmuFlight, and INAV.

## Table of Contents
- [Overview](#overview)
- [Features](#features)
- [Quick start (cli)](#quick-start-cli)
- [Output formats](#output-formats)
- [Smart export filtering](#smart-export-filtering)
- [Documentation](#documentation)
- [License](#license)
- [Acknowledgments](#acknowledgments)

## Overview

BBL Parser reads flight controller blackbox logs and provides a command-line interface to export CSV, GPX, and event data.

A Rust crate API is also available for programmatic access -- see [CRATE_USAGE.md](./CRATE_USAGE.md).

The CSV export matches blackbox-tools field order and naming, and decoding includes full P-frame predictor logic.

## Features

- Pure Rust parser (no external binaries)
- Command-line interface (CLI)
- Multi-log file support
- I, P, H, S, G, E frame decoding (reference-compliant)
- CSV export compatible with blackbox_decode
- GPX export for GPS tracks
- Event export (CLI)
- Streaming architecture suitable for large logs

## Quick start (cli)

```bash
# Build once
cargo build --release

# Analyze a file (console stats only)
./target/release/bbl_parser flight.BBL

# Export CSV / GPX / Events
./target/release/bbl_parser --csv --gpx --event logs/*.BBL

# Useful options
./target/release/bbl_parser logs/*.BBL --output-dir ./output
./target/release/bbl_parser --force-export logs/*.BBL
```

## Output formats

- CSV: main flight data `[.XX].csv` and headers `[.XX].headers.csv` (field order matches blackbox_decode; time column is "time (us)")
- GPX: GPS track `[.XX].gps.gpx`
- Events: JSON Lines `[.XX].event` (CLI)

Filenames are clean for single-log files and numbered for multi-log files (e.g., `.01.csv`, `.02.csv`).

## Smart export filtering

To reduce noise from test arm/disarm logs:
- < 5s: skipped
- 5–15s: exported only if data density > 1500 fps
- > 15s: exported

Use `--force-export` to export everything.

## Documentation

- Project overview: [OVERVIEW.md](./OVERVIEW.md)
- Frame details: [FRAMES.md](./FRAMES.md)
- Goals: [GOALS.md](./GOALS.md)
- Rust crate usage: [CRATE_USAGE.md](./CRATE_USAGE.md)
- Rust crate examples: [examples/README.md](./examples/README.md)

## License

Dual-licensed:
- Open source: [AGPL-3.0-or-later](./LICENSE)
- Commercial option: [LICENSE_COMMERCIAL](./LICENSE_COMMERCIAL)

## Acknowledgments

Inspired by Betaflight's [blackbox-log-viewer](https://github.com/betaflight/blackbox-log-viewer) and [blackbox-tools](https://github.com/betaflight/blackbox-tools).
