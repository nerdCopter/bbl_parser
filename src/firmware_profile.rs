//! Firmware vendor and version rules.
//!
//! This module answers one question: given the header `Firmware revision`
//! string, which vendor wrote the log and which name tables apply? The name
//! tables themselves live in [`crate::firmware_tables`]. All vendor and
//! version rules live here, as data ([`VENDORS`], [`PROFILE_RULES`]) and not
//! as string tests spread through the export code.
//!
//! # Detection rule
//!
//! The vendor is the FIRST whitespace-delimited word of the revision string,
//! compared ASCII case-insensitively against [`VENDORS`]. Text later in the
//! string never selects a vendor, so `Quicksilver 1.0 (INAV-compatible
//! build)` is Quicksilver, not INAV. The version is the second word, parsed
//! as semantic version; a pre-release suffix (`10.0.0-rc2`) is ignored.
//!
//! # Unknown vendors
//!
//! A first word that matches no vendor (and an empty string) gives
//! [`Vendor::Unknown`]. Unknown firmware keeps the Betaflight/EmuFlight name
//! tables, as before this module existed.
//!
//! # Rules not yet folded in
//!
//! [`FirmwareProfile`] is the place for every other per-vendor or
//! per-version rule. These rules still live elsewhere, because the changes
//! that rewrite them are not in this branch's history. Move each one here
//! after its change merges:
//! - battery voltage scale (`convert_vbat_to_volts`) and the vbat decimals
//!   and energy rules per vendor and version;
//! - INAV GPS altitude unit;
//! - amperage and energy scaling;
//! - event ids and event payload layouts per vendor.

use crate::firmware_tables::{
    InavVersion, FAILSAFE_PHASE_NAMES, INAV_FAILSAFE_PHASE_NAMES, INAV_FIRST_VERIFIED_VERSION,
};

/// `(major, minor, patch)` of a firmware revision.
pub type FirmwareVersion = InavVersion;

/// Firmware vendor named by a revision header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Betaflight,
    EmuFlight,
    Inav,
    Quicksilver,
    /// First word matches no known vendor, or the string is empty.
    Unknown,
}

/// One row of the vendor table.
pub struct VendorInfo {
    pub vendor: Vendor,
    /// First word of the revision header, compared ASCII case-insensitively.
    pub keyword: &'static str,
    /// Filename prefix used for exported files and BBL file names.
    pub file_prefix: &'static str,
    /// Human-readable vendor name.
    pub display_name: &'static str,
}

/// Known firmware vendors. To add a vendor: add a [`Vendor`] variant and one
/// row here, then add its rows to [`PROFILE_RULES`] if it needs its own tables.
pub const VENDORS: &[VendorInfo] = &[
    VendorInfo {
        vendor: Vendor::EmuFlight,
        keyword: "emuflight",
        file_prefix: "EMUF_",
        display_name: "EmuFlight",
    },
    VendorInfo {
        vendor: Vendor::Betaflight,
        keyword: "betaflight",
        file_prefix: "BTFL_",
        display_name: "Betaflight",
    },
    VendorInfo {
        vendor: Vendor::Inav,
        keyword: "inav",
        file_prefix: "INAV_",
        display_name: "iNav",
    },
    // Quicksilver writes Betaflight revision headers, so this row matches
    // only through its filename prefix in practice.
    VendorInfo {
        vendor: Vendor::Quicksilver,
        keyword: "quicksilver",
        file_prefix: "QUIC_",
        display_name: "Quicksilver",
    },
];

impl Vendor {
    /// Detect the vendor from the first word of a revision header.
    pub fn detect(revision: &str) -> Vendor {
        let Some(first) = revision.split_whitespace().next() else {
            return Vendor::Unknown;
        };
        VENDORS
            .iter()
            .find(|info| first.eq_ignore_ascii_case(info.keyword))
            .map_or(Vendor::Unknown, |info| info.vendor)
    }

    fn info(self) -> Option<&'static VendorInfo> {
        VENDORS.iter().find(|info| info.vendor == self)
    }

    /// Filename prefix (for example `"BTFL_"`); `None` for [`Vendor::Unknown`].
    pub fn file_prefix(self) -> Option<&'static str> {
        self.info().map(|info| info.file_prefix)
    }

    /// Human-readable name; `"Unknown"` for [`Vendor::Unknown`].
    pub fn display_name(self) -> &'static str {
        self.info().map_or("Unknown", |info| info.display_name)
    }

    /// Vendor for a filename prefix such as `"INAV_"`.
    pub fn from_file_prefix(prefix: &str) -> Vendor {
        VENDORS
            .iter()
            .find(|info| info.file_prefix == prefix)
            .map_or(Vendor::Unknown, |info| info.vendor)
    }
}

/// How a profile names the bits of the S-frame flag fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagNaming {
    /// Betaflight/EmuFlight tables (`FLIGHT_MODE_FLAG_NAMES`, `STATE_FLAG_NAMES`).
    Betaflight,
    /// INAV tables, filtered by the revision's version.
    Inav,
    /// No verified table: print the raw unsigned value.
    RawValue,
}

/// One row of [`PROFILE_RULES`]: which tables apply to a vendor and version.
pub struct ProfileRule {
    /// Vendor the rule applies to; `None` matches every vendor.
    pub vendor: Option<Vendor>,
    /// Lowest version the rule applies to. `Some` also requires a parsable
    /// version; `None` matches any version, including none.
    pub min_version: Option<FirmwareVersion>,
    pub failsafe_phase_names: &'static [&'static str],
    pub flag_naming: FlagNaming,
}

/// Profile rules. The first matching rule wins, so keep specific rows first
/// and the catch-all last.
pub const PROFILE_RULES: &[ProfileRule] = &[
    // INAV with a version that the INAV flag tables cover.
    ProfileRule {
        vendor: Some(Vendor::Inav),
        min_version: Some(INAV_FIRST_VERIFIED_VERSION),
        failsafe_phase_names: &INAV_FAILSAFE_PHASE_NAMES,
        flag_naming: FlagNaming::Inav,
    },
    // INAV with no parsable version, or older than the tables cover: the
    // Betaflight flag names are wrong and the INAV names are unverified.
    ProfileRule {
        vendor: Some(Vendor::Inav),
        min_version: None,
        failsafe_phase_names: &INAV_FAILSAFE_PHASE_NAMES,
        flag_naming: FlagNaming::RawValue,
    },
    // Betaflight, EmuFlight, Quicksilver and unknown firmware.
    ProfileRule {
        vendor: None,
        min_version: None,
        failsafe_phase_names: &FAILSAFE_PHASE_NAMES,
        flag_naming: FlagNaming::Betaflight,
    },
];

impl ProfileRule {
    fn matches(&self, vendor: Vendor, version: Option<FirmwareVersion>) -> bool {
        self.vendor.is_none_or(|rule_vendor| rule_vendor == vendor)
            && self
                .min_version
                .is_none_or(|min| version.is_some_and(|found| found >= min))
    }
}

/// Parse the version (second word) of a revision header.
fn parse_version(revision: &str) -> Option<FirmwareVersion> {
    let version = semver::Version::parse(revision.split_whitespace().nth(1)?).ok()?;
    Some((version.major, version.minor, version.patch))
}

/// Vendor, version and applicable tables for one firmware revision string.
#[derive(Debug, Clone, Copy)]
pub struct FirmwareProfile {
    pub vendor: Vendor,
    /// `None` when the second word is missing or not a semantic version.
    pub version: Option<FirmwareVersion>,
    pub failsafe_phase_names: &'static [&'static str],
    pub flag_naming: FlagNaming,
}

impl FirmwareProfile {
    /// Build the profile for a header `Firmware revision` value.
    pub fn from_revision(revision: &str) -> FirmwareProfile {
        let vendor = Vendor::detect(revision);
        let version = parse_version(revision);
        // The last rule has no vendor or version condition, so a match always exists.
        let rule = PROFILE_RULES
            .iter()
            .find(|rule| rule.matches(vendor, version))
            .unwrap_or(&PROFILE_RULES[PROFILE_RULES.len() - 1]);
        FirmwareProfile {
            vendor,
            version,
            failsafe_phase_names: rule.failsafe_phase_names,
            flag_naming: rule.flag_naming,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_real_header_shapes() {
        assert_eq!(
            Vendor::detect("Betaflight 4.5.1 (77d01ba3b) AT32F435M"),
            Vendor::Betaflight
        );
        assert_eq!(
            Vendor::detect("EmuFlight 0.5.0-dev (abc) STM32F405"),
            Vendor::EmuFlight
        );
        assert_eq!(
            Vendor::detect("INAV 8.0.0 (ae47bcba01) TARGET"),
            Vendor::Inav
        );
        assert_eq!(Vendor::detect("iNav 7.1.2"), Vendor::Inav);
        assert_eq!(Vendor::detect("Quicksilver 1.0"), Vendor::Quicksilver);
    }

    #[test]
    fn first_word_rule_adversarial() {
        assert_eq!(
            Vendor::detect("Quicksilver 1.0 (INAV-compatible build)"),
            Vendor::Quicksilver
        );
        assert_eq!(
            Vendor::detect("Betaflight 4.5.0 (INAV)"),
            Vendor::Betaflight
        );
        assert_eq!(Vendor::detect("   INAV 8.0.0"), Vendor::Inav);
        assert_eq!(Vendor::detect("iNaV 8.0.0"), Vendor::Inav);
        assert_eq!(Vendor::detect("INAV-compatible 1.0"), Vendor::Unknown);
        assert_eq!(Vendor::detect("Foo INAV 8.0.0"), Vendor::Unknown);
        assert_eq!(Vendor::detect(""), Vendor::Unknown);
        assert_eq!(Vendor::detect("   "), Vendor::Unknown);
        assert_eq!(Vendor::detect("Cleanflight 2.3.3"), Vendor::Unknown);
    }

    #[test]
    fn parses_version_from_second_word() {
        assert_eq!(parse_version("INAV 8.0.1 (sha) T"), Some((8, 0, 1)));
        assert_eq!(parse_version("INAV 10.0.0-rc2 (sha) T"), Some((10, 0, 0)));
        assert_eq!(parse_version("INAV"), None);
        assert_eq!(parse_version("INAV 7.1 (sha) T"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn profile_selects_tables() {
        let inav = FirmwareProfile::from_revision("INAV 8.0.1 (sha) T");
        assert_eq!(inav.flag_naming, FlagNaming::Inav);
        assert_eq!(inav.failsafe_phase_names, &INAV_FAILSAFE_PHASE_NAMES[..]);
        assert_eq!(inav.version, Some((8, 0, 1)));

        for rev in ["INAV", "INAV 5.1.0 (sha) T", "INAV 7.1 (sha) T"] {
            let old = FirmwareProfile::from_revision(rev);
            assert_eq!(old.flag_naming, FlagNaming::RawValue, "{rev}");
            assert_eq!(old.failsafe_phase_names, &INAV_FAILSAFE_PHASE_NAMES[..]);
        }

        for rev in [
            "Betaflight 4.5.1 (sha) T",
            "EmuFlight 0.4.3",
            "Quicksilver 1.0 (INAV-compatible build)",
            "Betaflight 4.5.0 (INAV)",
            "",
            "Cleanflight 2.3.3",
        ] {
            let other = FirmwareProfile::from_revision(rev);
            assert_eq!(other.flag_naming, FlagNaming::Betaflight, "{rev}");
            assert_eq!(other.failsafe_phase_names, &FAILSAFE_PHASE_NAMES[..]);
        }
    }

    #[test]
    fn vendor_prefix_round_trip() {
        for info in VENDORS {
            assert_eq!(info.vendor.file_prefix(), Some(info.file_prefix));
            assert_eq!(Vendor::from_file_prefix(info.file_prefix), info.vendor);
        }
        assert_eq!(Vendor::Unknown.file_prefix(), None);
        assert_eq!(Vendor::Unknown.display_name(), "Unknown");
        assert_eq!(Vendor::from_file_prefix("XXXX_"), Vendor::Unknown);
        assert_eq!(Vendor::Inav.display_name(), "iNav");
    }
}
