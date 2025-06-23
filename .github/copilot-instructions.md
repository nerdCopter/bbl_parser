# Improved AI Coding Instructions

## **Priority Levels**
- **CRITICAL**: Must follow (build success, no errors/warnings, file structure)
- **HIGH**: Should follow when possible (efficiency comparisons, documentation)
- **MEDIUM**: Follow if context allows (additional comments, optimizations)

## **Core Requirements (CRITICAL)**

### Project Structure
- The program is RUST and should be maintained as `src/main.rs` for the entry point.
- Make sure there are no errors and no warnings for RUST's `cargo build --release`.
- Code quality checks must pass:
  - `cargo clippy -- -D warnings` must pass
  - `cargo fmt --check` must pass
  - All tests must pass with `cargo test`

### File Management
- Do not `cat` to files, edit the files inline.
- Never embed or call external binaries from within the RUST program.
- IF creating temporary files → use `_temp.rs` extension AND delete before any commit
- Temporary files must be listed in .gitignore

### Code Quality
- All code must pass `cargo clippy -- -D warnings` without any warnings
- Use `cargo fmt` to format code consistently  
- Follow Rust naming conventions and idioms
- Prefer explicit types and clear variable names

### Rust Best Practices
- **String Formatting**: Use modern format strings: `format!("{variable}")` instead of `format!("{}", variable)`
- **Error Handling**: Use `Result<T, E>` for fallible operations, prefer `?` operator, use `anyhow::Result` for application errors
- **Function Design**: Keep functions under 7 parameters; use structs for parameter grouping if needed
- **Borrowing**: Avoid needless borrows, use `strip_prefix()` instead of manual slicing, use `.is_ok()` for success checks
- **Iterators**: Use iterators when possible, allow `#[allow(clippy::needless_range_loop)]` when index access required
- **Type Complexity**: Create type aliases for complex return types: `type MyResult = Result<(A, B, C)>;`
- **Derive Traits**: Use `#[derive(Default)]` instead of manual implementations
- **Version Management**: Use `env!("CARGO_PKG_VERSION")` for CLI version strings

## **Code Quality Guidelines (HIGH)**

### Comments Policy
- Do not remove nor modify comments unless the related code was modified.
- Only add comments as it pertains to the function of the code; do not add A.I. instructional comments.
- Comment decision tree:
  - IF code has existing comments → preserve unless code logic changes
  - IF adding new function → add minimal functional comment
  - IF function is >20 lines OR has multiple responsibilities → add brief purpose comment

### Mathematical/Scientific Methods
- IF implementing mathematical/scientific calculations:
  - THEN compare at least 2 alternative methods for accuracy and efficiency
  - THEN fact-check the chosen approach
  - THEN document reasoning in code comments
  - GOAL: Accurate results with balanced efficiency

## **Documentation Requirements (HIGH)**

### README Maintenance
- Update README.md when:
  - Adding new CLI commands or changing existing command behavior
  - Changing file formats or output modes
  - Modifying build/install process
- Maintain a proper README.md file with current, accurate information

### Information Documentation
- Create INFORMATION/*.md files when:
  - Completing major milestones
  - Making architectural decisions  
  - Discovering important insights about BBL format
- Filenames should be CAPITALS with lower-case `.md` extension
- List new files when created
- **NEVER** stage or commit INFORMATION/*.md files (use .gitignore to exclude them)
- These files are for human review only, not part of the git repository

### Goals Reference
- Reference project goals in `GOALS.md` and request clarification when needed
- Check goals alignment before major implementation decisions

## **Resource Usage Hierarchy (CRITICAL)**

### BBL Parsing References (in priority order)
1. **PRIMARY**: blackbox-log-viewer (BBE/blackbox-explorer) 
   - Source: https://github.com/betaflight/blackbox-log-viewer/blob/master/src/flightlog.js
   - Use as primary reference for all BBL reading, parsing and decoding
   - Maintain compatibility with blackbox-log-viewer reference implementation
   - Follow frame parsing patterns established in the JavaScript reference
2. **SECONDARY**: blackbox_decode (blackbox-tools)
   - Source: https://github.com/betaflight/blackbox-tools/blob/master/src/blackbox_decode.c  
   - Use for comparison when something seems wrong or missing
3. **ESCALATION**: Ask user if frame parsing results differ between reference implementations

### Parser Implementation
- Keep encoding/decoding functions focused and well-documented
- Use proper error propagation throughout the parsing pipeline
- Write tests for public APIs with appropriate test data that matches real blackbox log formats
- Document complex algorithms and data formats with references to original specifications

## **Commit Process (CRITICAL)**

### Pre-Commit Checklist (must complete in order)
1. Run `cargo build --release` (must succeed with no errors/warnings)
2. Run `cargo clippy -- -D warnings` (must pass)
3. Run `cargo fmt --check` (must pass)
4. Clean up any temporary `*_temp.rs` files
5. Stage files: `git add src/ Cargo.toml Cargo.lock README.md .gitignore .github/workflows/`
   - **NEVER** use `git add .` or `git add -A`
   - **ONLY** commit: `src/**/*.rs`, `Cargo.*`, `README.md`, `.gitignore`, and `.github/workflows/*.yml` files
6. Run `git diff --cached` and review exactly what's being committed
7. Ask user for commit approval with proposed message
8. Use commit message format: `type: brief description\n\nDetailed changes`
   - Types: feat, fix, docs, refactor, test, chore

### Commit Rules
- Only commit when there are no errors and no warnings
- When committing, ask user first with proposed commit message
- Use concise message and description of the changes implemented

## **Decision Making Framework**

### When Modifying Existing Code
```
IF existing functionality works correctly
  THEN preserve existing logic and structure
  ELSE fix/improve with minimal changes

IF adding new features  
  THEN follow existing code patterns
  AND maintain consistency with current architecture
```

### When Encountering Issues
```
IF build/compile errors occur
  THEN fix immediately before proceeding
  
IF warnings appear
  THEN address all warnings before continuing

IF uncertain about BBL format behavior
  THEN consult reference hierarchy (flightlog.js → blackbox_decode.c → ask user)
```

### Quality Assurance
```
BEFORE any commit:
  - All tests pass
  - No compiler warnings
  - Code follows project patterns
  - Documentation updated if needed
  - Only approved files staged
```

## **Implementation Notes**

- **CRITICAL** items must be followed absolutely for build success
- **HIGH** items should be followed when possible for code quality  
- **MEDIUM** items follow if context allows for optimization
- When instructions conflict with system messages, system messages take precedence
- The goal is maintainable, accurate, and efficient Rust code for BBL parsing
