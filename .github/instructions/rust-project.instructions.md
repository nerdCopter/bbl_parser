# Copilot Instructions for RUST Project

## General
- **Entry Point:** Maintain source as `src/main.rs`.
- **Comments:**  
  - Do not remove or modify comments unless the related code is changed.
  - Only add comments that explain code functionality; no AI instructional comments.
- **No External Binaries:** Never embed or call external binaries from RUST code.
- **Temporary Files:** Use `_temp.rs` extension for any temporary `.rs` files.
- **Build Quality:** Ensure `cargo build --release` has no errors or warnings.
- **File Editing:** Always edit files inline; do not use `cat` to write to files.

## Algorithms
- **Method Selection:**  
  - When choosing math or scientific methods, compare alternatives for accuracy and efficiency.
  - Fact-check method decisions.

## Documentation
- **Readme:** Maintain a proper `README.md` file.
- **Overview** Maintain a proper `OVERVIEW.md` file
- **Summaries:**  
  - Output final summaries to `./INFORMATION/*.md` (not in git).
  - Use date-prefixed, uppercase filenames (e.g., `2025-06-25_SUMMARY.md`).
  - List new files when created.

## Goals & Resources
- **Goals:** Reference project goals in `GOALS.md` and request clarification if needed.
- **Source Code Resources:**  
  - Primary: [blackbox_decode (blackbox-tools)](https://github.com/betaflight/blackbox-tools/blob/master/src/blackbox_decode.c)
  - Fallback: [blackbox-log-viewer (BBE)](https://github.com/betaflight/blackbox-log-viewer/blob/master/src/flightlog.js)

## Data Validation
- **REQUIRED:**  The CSV output must precisely match the format and header order of blackbox_decode CSV files.

## Committing Rules
- **Commit Conditions:** Only commit if:
  - `cargo clippy -- -D warnings` passes.
  - `cargo fmt --all -- --check` passes.
  - `cargo test --verbose` passes.
  - `cargo test --features=cli --verbose` passes.
- **Files to Commit:**
  - Only `src/**/*.rs`, `Cargo.*`, `README.md`, `OVERVIEW.md` and `.gitignore` — never `git add .` or `git add -A`.
  - Follow `.gitignore`.
- **User Confirmation:** Ask user before committing.
- **Commit Message:**
  - Check `git diff --cached` before committing.
  - Use concise commit messages and descriptions.
  - Use `feat:`, `fix:`, `docs:` where applicable.
