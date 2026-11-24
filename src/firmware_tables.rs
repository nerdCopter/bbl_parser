//! Firmware-specific constants and name tables used when writing CSV output.
//!
//! Every value in this module mirrors an upstream definition. Each section
//! names its source. Add new firmware tables as new sections; keep each table
//! next to the constants that index it.
//!
//! Upstream references:
//! - blackbox-tools (`blackbox_decode`): `src/blackbox_fielddefs.{c,h}` at
//!   `49e18cac31e657f9bdb1ecccc32bd401803f66f6` (master, includes PR 75).
//! - Betaflight: `src/main/flight/failsafe.h` at `60abbac05` (master).
//! - EmuFlight: `src/main/flight/failsafe.h` at
//!   `30eff58a0f7e3c7f07acc5bdd131f73801ce2f66`.

// ============================================================================
// Slow (S) frame field names
// ============================================================================

/// S-frame field that holds the flight-mode bit mask.
pub const FIELD_FLIGHT_MODE_FLAGS: &str = "flightModeFlags";
/// S-frame field that holds the state bit mask.
pub const FIELD_STATE_FLAGS: &str = "stateFlags";
/// S-frame field that holds the failsafe phase enum value.
pub const FIELD_FAILSAFE_PHASE: &str = "failsafePhase";

// ============================================================================
// Flight-mode flag names (`flightModeFlags`)
// ============================================================================

/// Names for each `flightModeFlags` bit, indexed by bit position.
///
/// Bits 0..=10 are `FLIGHT_LOG_FLIGHT_MODE_NAME` in blackbox-tools
/// (`blackbox_fielddefs.c`). Bit 11 (`GPS_RESCUE_MODE`) is a bbl_parser
/// addition; blackbox_decode does not print bits above its (stale) count of
/// 10, so it never prints `FAILSAFE_MODE` (bit 10) or bit 11.
pub const FLIGHT_MODE_FLAG_NAMES: [&str; 12] = [
    "ANGLE_MODE",       // bit 0
    "HORIZON_MODE",     // bit 1
    "MAG",              // bit 2, MAG_MODE
    "BARO",             // bit 3, ALT_HOLD_MODE in current firmware
    "GPS_HOME",         // bit 4
    "GPS_HOLD",         // bit 5, POS_HOLD_MODE in current firmware
    "HEADFREE",         // bit 6
    "UNUSED",           // bit 7, old autotune
    "PASSTHRU",         // bit 8
    "RANGEFINDER_MODE", // bit 9
    "FAILSAFE_MODE",    // bit 10
    "GPS_RESCUE_MODE",  // bit 11
];

// ============================================================================
// State flag names (`stateFlags`)
// ============================================================================

/// Names for each `stateFlags` bit, indexed by bit position.
///
/// Mirrors `FLIGHT_LOG_FLIGHT_STATE_NAME` (`FLIGHT_LOG_FLIGHT_STATE_COUNT` is 5).
pub const STATE_FLAG_NAMES: [&str; 5] = [
    "GPS_FIX_HOME",  // bit 0
    "GPS_FIX",       // bit 1
    "CALIBRATE_MAG", // bit 2, GPS_FIX_EVER in current firmware
    "SMALL_ANGLE",   // bit 3
    "FIXED_WING",    // bit 4
];

// ============================================================================
// Failsafe phase (`failsafePhase`)
// ============================================================================

/// `FAILSAFE_IDLE`
pub const FAILSAFE_IDLE: i32 = 0;
/// `FAILSAFE_RX_LOSS_DETECTED`
pub const FAILSAFE_RX_LOSS_DETECTED: i32 = 1;
/// `FAILSAFE_LANDING`
pub const FAILSAFE_LANDING: i32 = 2;
/// `FAILSAFE_LANDED`
pub const FAILSAFE_LANDED: i32 = 3;
/// `FAILSAFE_RX_LOSS_MONITORING`
pub const FAILSAFE_RX_LOSS_MONITORING: i32 = 4;
/// `FAILSAFE_RX_LOSS_RECOVERED`
pub const FAILSAFE_RX_LOSS_RECOVERED: i32 = 5;
/// `FAILSAFE_GPS_RESCUE` (Betaflight 3.4.0 and later, EmuFlight)
pub const FAILSAFE_GPS_RESCUE: i32 = 6;
/// `FAILSAFE_AUTOPILOT` (Betaflight 2026.6.0 and later; EmuFlight never logs it)
pub const FAILSAFE_AUTOPILOT: i32 = 7;

/// Names for each `failsafePhase` value, indexed by value.
///
/// Mirrors `FLIGHT_LOG_FAILSAFE_PHASE_NAME` (`FLIGHT_LOG_FAILSAFE_PHASE_COUNT`
/// is 8) in blackbox-tools after PR 75. Entries 4 and 5 keep the upstream
/// `FAILSAFE_` prefix. Betaflight and EmuFlight define values 0..=6 the same
/// way; only Betaflight defines 7. A value outside this table is printed as a
/// raw integer, as `flightlogDecodeEnumToString` does.
pub const FAILSAFE_PHASE_NAMES: [&str; 8] = [
    "IDLE",                        // FAILSAFE_IDLE
    "RX_LOSS_DETECTED",            // FAILSAFE_RX_LOSS_DETECTED
    "LANDING",                     // FAILSAFE_LANDING
    "LANDED",                      // FAILSAFE_LANDED
    "FAILSAFE_RX_LOSS_MONITORING", // FAILSAFE_RX_LOSS_MONITORING
    "FAILSAFE_RX_LOSS_RECOVERED",  // FAILSAFE_RX_LOSS_RECOVERED
    "GPS_RESCUE",                  // FAILSAFE_GPS_RESCUE
    "AUTOPILOT",                   // FAILSAFE_AUTOPILOT
];

// ============================================================================
// Event frame type ids (`E` frames)
// ============================================================================

/// `FLIGHT_LOG_EVENT_SYNC_BEEP`
pub const EVENT_SYNC_BEEP: u8 = 0;
/// `FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_START` (old numbering)
pub const EVENT_AUTOTUNE_CYCLE_START: u8 = 1;
/// `FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_RESULT` (old numbering)
pub const EVENT_AUTOTUNE_CYCLE_RESULT: u8 = 2;
/// `FLIGHT_LOG_EVENT_AUTOTUNE_TARGETS` (old numbering)
pub const EVENT_AUTOTUNE_TARGETS: u8 = 3;
/// `FLIGHT_LOG_EVENT_INFLIGHT_ADJUSTMENT` (old numbering)
pub const EVENT_INFLIGHT_ADJUSTMENT_OLD: u8 = 4;
/// `FLIGHT_LOG_EVENT_LOGGING_RESUME` (old numbering)
pub const EVENT_LOGGING_RESUME_OLD: u8 = 5;
/// `FLIGHT_LOG_EVENT_LOG_END` (old numbering)
pub const EVENT_LOG_END_OLD: u8 = 6;
/// Unused autotune cycle start (new numbering)
pub const EVENT_AUTOTUNE_CYCLE_START_UNUSED: u8 = 10;
/// Unused autotune cycle result (new numbering)
pub const EVENT_AUTOTUNE_CYCLE_RESULT_UNUSED: u8 = 11;
/// Unused autotune targets (new numbering)
pub const EVENT_AUTOTUNE_TARGETS_UNUSED: u8 = 12;
/// `FLIGHT_LOG_EVENT_INFLIGHT_ADJUSTMENT`
pub const EVENT_INFLIGHT_ADJUSTMENT: u8 = 13;
/// `FLIGHT_LOG_EVENT_LOGGING_RESUME`
pub const EVENT_LOGGING_RESUME: u8 = 14;
/// `FLIGHT_LOG_EVENT_DISARM`: payload is one unsigned VB, the disarm reason.
///
/// Betaflight only. Betaflight `blackbox.c` at `60abbac05` writes it in
/// `blackboxLogEvent`. EmuFlight (`30eff58a0`) and INAV (`bbc055369`) do not
/// define or write id 15, so there is no conflicting meaning to gate on.
pub const EVENT_DISARM: u8 = 15;
/// `FLIGHT_LOG_EVENT_FLIGHTMODE`: payload is two unsigned VBs, `flags` then
/// `lastFlags`. Betaflight, EmuFlight and INAV write the same layout.
pub const EVENT_FLIGHTMODE: u8 = 30;
/// `FLIGHT_LOG_EVENT_LOG_END`
pub const EVENT_LOG_END: u8 = 255;

/// Event name printed by blackbox_decode for `FLIGHT_LOG_EVENT_DISARM`.
pub const EVENT_NAME_DISARM: &str = "Disarm";
/// Event name printed by blackbox_decode for `FLIGHT_LOG_EVENT_FLIGHTMODE`.
pub const EVENT_NAME_FLIGHT_MODE: &str = "Flight mode";
