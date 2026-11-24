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

/// Names for each INAV `failsafePhase` value, indexed by value.
///
/// Mirrors `failsafePhase_e` in INAV `src/main/flight/failsafe.h` without the
/// `FAILSAFE_` prefix (identical in INAV 6.1.1 through master). The order
/// differs from Betaflight and EmuFlight. blackbox_decode has no INAV table,
/// so this has no upstream decoder parity target.
pub const INAV_FAILSAFE_PHASE_NAMES: [&str; 8] = [
    "IDLE",               // 0
    "RX_LOSS_DETECTED",   // 1
    "RX_LOSS_IDLE",       // 2
    "RETURN_TO_HOME",     // 3
    "LANDING",            // 4
    "LANDED",             // 5
    "RX_LOSS_MONITORING", // 6
    "RX_LOSS_RECOVERED",  // 7
];

// ============================================================================
// INAV flag names (`flightModeFlags`, `flightModeFlags2`,
// `activeFlightModeFlags`, `stateFlags`)
// ============================================================================
//
// INAV does not log the Betaflight flight-mode enum. Verified against the
// INAV source (`git show <ref>:<path>`, read-only), refs and shas:
//   6.0.0 7e7efe4a54, 6.1.0 d6ce6e7014, 6.1.1 42ced253bd, 7.0.0 118cd48550,
//   7.1.0 aa8543654d, 7.1.2 ed7fb3bb32, 8.0.0 ec2106af46, 8.0.1 ae47bcba01,
//   9.0.0 a7932b92ea, 9.0.1 d44f2cf6e6, 9.1.0 e519b69b02,
//   10.0.0-rc2 f1cd5c6f68, upstream/maintenance-8.x.x 368bd37600,
//   upstream/maintenance-9.x f1558a086f, upstream/release/9.1 305d304f23,
//   upstream/maintenance-10.x 2527f7764f, upstream/maintenance-11.x
//   7e82f68050, upstream/master 940b9281bb.
//
// - `flightModeFlags` is the first 32 bits of `rcModeActivationMask`, indexed
//   by `boxId_e` (`src/main/blackbox/blackbox.c`: 6.1.1:1300, 7.1.2:1310,
//   8.0.1:1385, 9.1.0:1371, 10.0.0-rc2:1514 and 1511). 8.0.0 and later also
//   log `flightModeFlags2` (box bits 32..=63, 8.0.1:1386) and
//   `activeFlightModeFlags` (the `flightModeFlags_e` runtime mask, 8.0.1:1392).
// - `boxId_e` is in `src/main/fc/rc_modes.h` (6.1.1:28-81, 7.0.0:28-85,
//   7.1.2:28-86, 8.0.1:28-89, 10.0.0-rc2:28-93).
// - `flightModeFlags_e` and `stateFlags_t` are in
//   `src/main/fc/runtime_config.h` (6.1.1:107 and 142, 7.1.2:109 and 146,
//   8.0.1:111 and 151, 10.0.0-rc2:111 and 151).
// - Version keys come from the release tags. The `since`/`until` values
//   below are the tags where the enum sets change; all other tags from 6.0.0
//   to 10.0.0-rc2 match their neighbours. Branch heads that report the same
//   version as a tag (master and release/9.1 report 9.1.1; maintenance-11.x
//   reports 9.1.0 yet adds box bits 60..=62) are not separate keys: maintenance
//   11.x bits 60..=62 stay unnamed under a 9.1.x version.
// - Not verified: releases below 6.0.0 (the 5.x box enum has 48 entries), the
//   6.0.0-FP1 preview tag (50 box entries), pre-release builds between tags,
//   and any version above 10.0.0 (bits 60..=63 are named as in 10.0.0-rc2).

/// S-frame field with the box bits 32..=63 (INAV 8.0.0 and later).
pub const FIELD_FLIGHT_MODE_FLAGS2: &str = "flightModeFlags2";
/// S-frame field with the runtime `flightModeFlags_e` mask (INAV 8.0.0 and later).
pub const FIELD_ACTIVE_FLIGHT_MODE_FLAGS: &str = "activeFlightModeFlags";

/// An INAV version as `(major, minor, patch)`.
pub type InavVersion = (u64, u64, u64);

/// Oldest INAV version whose flag enums are verified (tags 6.0.0 to 6.1.1 are identical).
pub const INAV_FIRST_VERIFIED_VERSION: InavVersion = (6, 0, 0);
const INAV_V7_0_0: InavVersion = (7, 0, 0);
const INAV_V7_1_0: InavVersion = (7, 1, 0);
const INAV_V8_0_0: InavVersion = (8, 0, 0);
const INAV_V10_0_0: InavVersion = (10, 0, 0);

/// One INAV flag name and the version range in which it is defined.
#[derive(Debug, Clone, Copy)]
pub struct InavFlagName {
    /// Bit position inside the 32-bit logged word (0..=31).
    pub bit: u8,
    /// Enum name without the `BOX` prefix for box modes.
    pub name: &'static str,
    /// First version that defines this bit.
    pub since: InavVersion,
    /// First version that no longer defines this bit; `None` while still defined.
    pub until: Option<InavVersion>,
}

const fn flag(bit: u8, name: &'static str, since: InavVersion) -> InavFlagName {
    InavFlagName {
        bit,
        name,
        since,
        until: None,
    }
}

impl InavFlagName {
    /// True when `version` lies inside the range this entry is defined for.
    pub fn is_defined_in(&self, version: InavVersion) -> bool {
        version >= self.since && self.until.is_none_or(|until| version < until)
    }
}

/// `boxId_e` names for bits 0..=31 (`flightModeFlags`), from `rc_modes.h`.
///
/// Bit 23 `KILLSWITCH` exists in 6.0.0 to 7.1.2 and is gone in 8.0.0 (the bit
/// stays unused in later versions).
pub const INAV_BOX_MODE_NAMES: [InavFlagName; 32] = [
    flag(0, "ARM", INAV_FIRST_VERIFIED_VERSION),
    flag(1, "ANGLE", INAV_FIRST_VERIFIED_VERSION),
    flag(2, "HORIZON", INAV_FIRST_VERIFIED_VERSION),
    flag(3, "NAVALTHOLD", INAV_FIRST_VERIFIED_VERSION),
    flag(4, "HEADINGHOLD", INAV_FIRST_VERIFIED_VERSION),
    flag(5, "HEADFREE", INAV_FIRST_VERIFIED_VERSION),
    flag(6, "HEADADJ", INAV_FIRST_VERIFIED_VERSION),
    flag(7, "CAMSTAB", INAV_FIRST_VERIFIED_VERSION),
    flag(8, "NAVRTH", INAV_FIRST_VERIFIED_VERSION),
    flag(9, "NAVPOSHOLD", INAV_FIRST_VERIFIED_VERSION),
    flag(10, "MANUAL", INAV_FIRST_VERIFIED_VERSION),
    flag(11, "BEEPERON", INAV_FIRST_VERIFIED_VERSION),
    flag(12, "LEDLOW", INAV_FIRST_VERIFIED_VERSION),
    flag(13, "LIGHTS", INAV_FIRST_VERIFIED_VERSION),
    flag(14, "NAVLAUNCH", INAV_FIRST_VERIFIED_VERSION),
    flag(15, "OSD", INAV_FIRST_VERIFIED_VERSION),
    flag(16, "TELEMETRY", INAV_FIRST_VERIFIED_VERSION),
    flag(17, "BLACKBOX", INAV_FIRST_VERIFIED_VERSION),
    flag(18, "FAILSAFE", INAV_FIRST_VERIFIED_VERSION),
    flag(19, "NAVWP", INAV_FIRST_VERIFIED_VERSION),
    flag(20, "AIRMODE", INAV_FIRST_VERIFIED_VERSION),
    flag(21, "HOMERESET", INAV_FIRST_VERIFIED_VERSION),
    flag(22, "GCSNAV", INAV_FIRST_VERIFIED_VERSION),
    InavFlagName {
        bit: 23,
        name: "KILLSWITCH",
        since: INAV_FIRST_VERIFIED_VERSION,
        until: Some(INAV_V8_0_0),
    },
    flag(24, "SURFACE", INAV_FIRST_VERIFIED_VERSION),
    flag(25, "FLAPERON", INAV_FIRST_VERIFIED_VERSION),
    flag(26, "TURNASSIST", INAV_FIRST_VERIFIED_VERSION),
    flag(27, "AUTOTRIM", INAV_FIRST_VERIFIED_VERSION),
    flag(28, "AUTOTUNE", INAV_FIRST_VERIFIED_VERSION),
    flag(29, "CAMERA1", INAV_FIRST_VERIFIED_VERSION),
    flag(30, "CAMERA2", INAV_FIRST_VERIFIED_VERSION),
    flag(31, "CAMERA3", INAV_FIRST_VERIFIED_VERSION),
];

/// `boxId_e` names for bits 32..=63 (`flightModeFlags2`, INAV 8.0.0 and later).
///
/// `bit` is the position inside the second word (box id minus 32). Entries
/// below 8.0.0 are listed for completeness; the field exists only from 8.0.0.
/// Bits 28..=31 (box ids 60..=63) exist from 10.0.0 (`AUTOSPEED`,
/// `TERRAINAGLHOLD`, `INFLIGHTMENU`, `THRUSTREVERSE`).
pub const INAV_BOX_MODE_NAMES_HIGH: [InavFlagName; 32] = [
    flag(0, "OSDALT1", INAV_FIRST_VERIFIED_VERSION),
    flag(1, "OSDALT2", INAV_FIRST_VERIFIED_VERSION),
    flag(2, "OSDALT3", INAV_FIRST_VERIFIED_VERSION),
    flag(3, "NAVCOURSEHOLD", INAV_FIRST_VERIFIED_VERSION),
    flag(4, "BRAKING", INAV_FIRST_VERIFIED_VERSION),
    flag(5, "USER1", INAV_FIRST_VERIFIED_VERSION),
    flag(6, "USER2", INAV_FIRST_VERIFIED_VERSION),
    flag(7, "FPVANGLEMIX", INAV_FIRST_VERIFIED_VERSION),
    flag(8, "LOITERDIRCHN", INAV_FIRST_VERIFIED_VERSION),
    flag(9, "MSPRCOVERRIDE", INAV_FIRST_VERIFIED_VERSION),
    flag(10, "PREARM", INAV_FIRST_VERIFIED_VERSION),
    flag(11, "TURTLE", INAV_FIRST_VERIFIED_VERSION),
    flag(12, "NAVCRUISE", INAV_FIRST_VERIFIED_VERSION),
    flag(13, "AUTOLEVEL", INAV_FIRST_VERIFIED_VERSION),
    flag(14, "PLANWPMISSION", INAV_FIRST_VERIFIED_VERSION),
    flag(15, "SOARING", INAV_FIRST_VERIFIED_VERSION),
    flag(16, "USER3", INAV_FIRST_VERIFIED_VERSION),
    flag(17, "USER4", INAV_FIRST_VERIFIED_VERSION),
    flag(18, "CHANGEMISSION", INAV_FIRST_VERIFIED_VERSION),
    flag(19, "BEEPERMUTE", INAV_V7_0_0),
    flag(20, "MULTIFUNCTION", INAV_V7_0_0),
    flag(21, "MIXERPROFILE", INAV_V7_0_0),
    flag(22, "MIXERTRANSITION", INAV_V7_0_0),
    flag(23, "ANGLEHOLD", INAV_V7_1_0),
    flag(24, "GIMBALTLOCK", INAV_V8_0_0),
    flag(25, "GIMBALRLOCK", INAV_V8_0_0),
    flag(26, "GIMBALCENTER", INAV_V8_0_0),
    flag(27, "GIMBALHTRK", INAV_V8_0_0),
    flag(28, "AUTOSPEED", INAV_V10_0_0),
    flag(29, "TERRAINAGLHOLD", INAV_V10_0_0),
    flag(30, "INFLIGHTMENU", INAV_V10_0_0),
    flag(31, "THRUSTREVERSE", INAV_V10_0_0),
];

/// `flightModeFlags_e` names (`activeFlightModeFlags`, INAV 8.0.0 and later), from `runtime_config.h`.
pub const INAV_ACTIVE_FLIGHT_MODE_NAMES: [InavFlagName; 20] = [
    flag(0, "ANGLE_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(1, "HORIZON_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(2, "HEADING_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(3, "NAV_ALTHOLD_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(4, "NAV_RTH_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(5, "NAV_POSHOLD_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(6, "HEADFREE_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(7, "NAV_LAUNCH_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(8, "MANUAL_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(9, "FAILSAFE_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(10, "AUTO_TUNE", INAV_FIRST_VERIFIED_VERSION),
    flag(11, "NAV_WP_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(12, "NAV_COURSE_HOLD_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(13, "FLAPERON", INAV_FIRST_VERIFIED_VERSION),
    flag(14, "TURN_ASSISTANT", INAV_FIRST_VERIFIED_VERSION),
    flag(15, "TURTLE_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(16, "SOARING_MODE", INAV_FIRST_VERIFIED_VERSION),
    flag(17, "ANGLEHOLD_MODE", INAV_V7_1_0),
    flag(18, "NAV_FW_AUTOLAND", INAV_V7_1_0),
    flag(19, "NAV_SEND_TO", INAV_V8_0_0),
];

/// `stateFlags_t` names (`stateFlags`), from `runtime_config.h`.
///
/// Bit 4 is `FIXED_WING_LEGACY` (Betaflight table: `FIXED_WING`). Bit 10
/// exists from 8.0.0, bit 27 from 7.0.0, bit 28 from 7.1.0.
pub const INAV_STATE_FLAG_NAMES: [InavFlagName; 29] = [
    flag(0, "GPS_FIX_HOME", INAV_FIRST_VERIFIED_VERSION),
    flag(1, "GPS_FIX", INAV_FIRST_VERIFIED_VERSION),
    flag(2, "CALIBRATE_MAG", INAV_FIRST_VERIFIED_VERSION),
    flag(3, "SMALL_ANGLE", INAV_FIRST_VERIFIED_VERSION),
    flag(4, "FIXED_WING_LEGACY", INAV_FIRST_VERIFIED_VERSION),
    flag(5, "ANTI_WINDUP", INAV_FIRST_VERIFIED_VERSION),
    flag(6, "FLAPERON_AVAILABLE", INAV_FIRST_VERIFIED_VERSION),
    flag(7, "NAV_MOTOR_STOP_OR_IDLE", INAV_FIRST_VERIFIED_VERSION),
    flag(8, "COMPASS_CALIBRATED", INAV_FIRST_VERIFIED_VERSION),
    flag(9, "ACCELEROMETER_CALIBRATED", INAV_FIRST_VERIFIED_VERSION),
    flag(10, "GPS_ESTIMATED_FIX", INAV_V8_0_0),
    flag(11, "NAV_CRUISE_BRAKING", INAV_FIRST_VERIFIED_VERSION),
    flag(12, "NAV_CRUISE_BRAKING_BOOST", INAV_FIRST_VERIFIED_VERSION),
    flag(13, "NAV_CRUISE_BRAKING_LOCKED", INAV_FIRST_VERIFIED_VERSION),
    flag(
        14,
        "NAV_EXTRA_ARMING_SAFETY_BYPASSED",
        INAV_FIRST_VERIFIED_VERSION,
    ),
    flag(15, "AIRMODE_ACTIVE", INAV_FIRST_VERIFIED_VERSION),
    flag(16, "ESC_SENSOR_ENABLED", INAV_FIRST_VERIFIED_VERSION),
    flag(17, "AIRPLANE", INAV_FIRST_VERIFIED_VERSION),
    flag(18, "MULTIROTOR", INAV_FIRST_VERIFIED_VERSION),
    flag(19, "ROVER", INAV_FIRST_VERIFIED_VERSION),
    flag(20, "BOAT", INAV_FIRST_VERIFIED_VERSION),
    flag(21, "ALTITUDE_CONTROL", INAV_FIRST_VERIFIED_VERSION),
    flag(22, "MOVE_FORWARD_ONLY", INAV_FIRST_VERIFIED_VERSION),
    flag(
        23,
        "SET_REVERSIBLE_MOTORS_FORWARD",
        INAV_FIRST_VERIFIED_VERSION,
    ),
    flag(24, "FW_HEADING_USE_YAW", INAV_FIRST_VERIFIED_VERSION),
    flag(25, "ANTI_WINDUP_DEACTIVATED", INAV_FIRST_VERIFIED_VERSION),
    flag(26, "LANDING_DETECTED", INAV_FIRST_VERIFIED_VERSION),
    flag(27, "IN_FLIGHT_EMERG_REARM", INAV_V7_0_0),
    flag(28, "TAILSITTER", INAV_V7_1_0),
];

// ============================================================================
// Event type ids (reserved for a later change)
// ============================================================================
