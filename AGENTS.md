# Copilot Instructions for RUST Project

## General
- **Entry Point:** Maintain CLI source as `src/main.rs` and library core as `src/lib.rs`.
- **Binary Structure:** CLI uses feature flag `cli` and entry point is built via default features or explicit `--features=cli`.
- **Comments:**  
  - Do not remove or modify comments unless the related code is changed.
  - Only add comments that explain code functionality; no AI instructional comments.
- **No External Binaries:** Never embed or call external binaries from RUST code.
- **Temporary Files:** Use `_temp.rs` extension for any temporary `.rs` files.
- **Build Quality:** Ensure `cargo build --release` has no errors or warnings.
- **File Editing:** Always edit files inline; do not use `cat` to write to files.

## Architecture & Code Organization
- **Library-first design:** Core logic in `src/lib.rs` and CLI entry point in `src/main.rs`.
  - **Shared code:** Parser modules (`src/parser/`) and export functions (`src/export.rs`) are shared by both library and CLI.
  - **CLI as thin wrapper:** The CLI (`src/main.rs`) uses library export functions (`export_to_csv`, `export_to_gpx`, `export_to_event`) with CLI-specific status messages.
  - **Current state:** **Full unification complete** — parsing and export layers unified, CLI reduced from ~1800 to ~1400 lines.
- **Decision criteria:** "Is this needed by crate consumers?" determines placement — shared logic in library, CLI-only logic in `src/main.rs`.
- **Feature flags:** `csv`, `cli`, `json`, `serde` control optional dependencies; default: `csv` + `cli`.
- **CRATE_USAGE.md reference:** See `CRATE_USAGE.md` for library API examples with feature flags.
- **Testing:** Comprehensive tests distributed across `src/main.rs`, `src/conversion.rs`, `src/parser/stream.rs`, and `src/parser/helpers.rs`.
- **Public API:** `parse_bbl_file()`, `parse_bbl_bytes()`, `BBLLog`, `ExportOptions`, `export_to_csv()`, `export_to_gpx()`, `export_to_event()`, conversion utilities, parser helpers.

## Algorithms
- **Method Selection:**  
  - When choosing math or scientific methods, compare alternatives for accuracy and efficiency.
  - Fact-check method decisions.

## Documentation
- **Readme:** Maintain a proper `README.md` file.
- **Overview:** Maintain a proper `OVERVIEW.md` file
- **Summaries:**  
  - Output final summaries to `./INFORMATION/*.md` (not in git).
  - Use date-prefixed, uppercase filenames (e.g., `2025-06-25_SUMMARY.md`).
  - List new files when created.

## Development Environment
- **Setup Script:** Use `.github/setup-dev.sh` for automated development environment setup
- **Pre-commit Hook:** `.github/pre-commit-hook.sh` automatically formats code before commits
- **CI Compliance:** All changes must pass the GitHub Actions CI pipeline

## Goals & Resources
- **Goals:** Reference project goals in `GOALS.md` and request clarification if needed.
- **Source Code Resources:**  
  - Primary: [blackbox_decode (blackbox-tools)](https://github.com/betaflight/blackbox-tools/blob/master/src/blackbox_decode.c)
  - Fallback: [blackbox-log-viewer (BBE)](https://github.com/betaflight/blackbox-log-viewer/blob/master/src/flightlog.js)

## Data Validation
- **REQUIRED:**  The CSV output must precisely match the format and header order of blackbox_decode CSV files.

## GitHub Actions & Workflow Design
- **Prefer native tooling:** Use GitHub Actions features, `git` CLI, `gh` CLI (not third-party actions).
- **Reuse artifacts:** Build once (ci.yml), download in downstream jobs (release.yml) — ~8-10x faster, lower costs.
- **Explicit dependencies:** Use `needs:` clauses for job ordering; validate upstream success before proceeding.
- **Error handling:** Validate artifacts exist; use `set -e` in scripts; provide clear error messages.

## Committing Rules
- **Commit Conditions:** Only commit if:
  - `cargo clippy --all-targets --all-features -- -D warnings` passes.
  - `cargo fmt --all -- --check` passes.
  - `cargo test --verbose` passes.
  - `cargo test --features=cli --verbose` passes.
  - `cargo build --release` passes with no errors or warnings.
- **Files to Commit:**
  - Only `src/**/*.rs`, `Cargo.*`, `README.md`, `OVERVIEW.md`, `.gitignore`, and `.github/**` — never `git add .` or `git add -A`.
  - Follow `.gitignore`.
- **User Confirmation:** Ask user before committing.
- **Commit Message:**
  - Check `git diff --cached` before committing.
  - Use concise commit messages and descriptions.
  - Use `feat:`, `fix:`, `docs:` where applicable.

## Mandatory Checks
- **BEFORE ANY CODE CHANGES:** Always run `cargo clippy --all-targets --all-features -- -D warnings` to catch ALL issues.
- **BEFORE ANY CODE CHANGES:** Always run `cargo fmt --all -- --check` to ensure formatting compliance.
- **IMMEDIATE FORMATTING FIX:** If `cargo fmt --all -- --check` fails, IMMEDIATELY run `cargo fmt --all` to fix formatting.
- **NO OPTIONAL FEATURES ERRORS:** All feature combinations must compile without errors.
- **NO FORMATTING VIOLATIONS:** Code must pass `cargo fmt --all -- --check` without any formatting issues.
- **STRICT COMPLIANCE:** Never skip clippy checks or formatting checks. Never allow warnings to pass.
- **AUTOMATIC FORMATTING:** ALWAYS run `cargo fmt --all` after making ANY code changes before moving to next steps.
- **DOUBLE CHECK FORMATTING:** After running `cargo fmt --all`, ALWAYS verify with `cargo fmt --all -- --check` before proceeding.
