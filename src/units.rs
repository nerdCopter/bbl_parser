//! User-selectable output units for the flight-data CSV and GPX export.
//!
//! Mirrors the `--unit-*` and `--alt-offset` options of `blackbox_decode`
//! (blackbox-tools `src/blackbox_decode.c`, `src/units.c` at `d89f96b`).
//! Every unit enum here lists only the names that apply to its field, so an
//! unusable combination (for example volts for altitude) cannot be built.
//! The default [`UnitOptions`] reproduce the historic CSV output exactly.

use crate::firmware_tables::{
    ACCELERATION_DUE_TO_GRAVITY, FEET_PER_METER, HEADER_ACC_1G, HEADER_FIRMWARE_TYPE,
    HEADER_GYRO_SCALE, HEADER_GYRO_SCALE_DOTTED, MICROSECONDS_PER_SECOND,
    NATIVE_RADIANS_FIRMWARE_TYPE,
};
use crate::types::BBLHeader;
use std::str::FromStr;

macro_rules! unit_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $label:expr, $header:expr),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Every accepted unit name, in the order they are documented.
            pub const NAMES: &'static [&'static str] = &[$($label),+];

            /// Unit text in the CSV header, `None` for [`Self::Raw`]. Matches
            /// the upstream `UNIT_NAME` table, which differs from the accepted name for m/s2.
            pub fn label(self) -> Option<&'static str> {
                match self {
                    $(Self::$variant => if $label == "raw" { None } else { Some($header) }),+
                }
            }
        }

        impl FromStr for $name {
            type Err = String;

            /// Case-insensitive, like `unitFromName` upstream.
            fn from_str(text: &str) -> Result<Self, String> {
                let lower = text.to_ascii_lowercase();
                $(
                    if lower == $label.to_ascii_lowercase() {
                        return Ok(Self::$variant);
                    }
                )+
                Err(format!(
                    "invalid unit '{}', expected one of: {}",
                    text,
                    Self::NAMES.join(", ")
                ))
            }
        }
    };
}

unit_enum! {
    /// Unit of `vbatLatest`.
    VbatUnit { Raw => "raw", "raw", MilliVolts => "mV", "mV", Volts => "V", "V" }
}
unit_enum! {
    /// Unit of `amperageLatest`.
    AmperageUnit { Raw => "raw", "raw", MilliAmps => "mA", "mA", Amps => "A", "A" }
}
unit_enum! {
    /// Form of the flight-mode, state and failsafe-phase fields.
    FlagsUnit { Raw => "raw", "raw", Flags => "flags", "flags" }
}
unit_enum! {
    /// Unit of the `time` field.
    FrameTimeUnit { Raw => "raw", "raw", Microseconds => "us", "us", Milliseconds => "ms", "ms", Seconds => "s", "s" }
}
unit_enum! {
    /// Unit of `BaroAlt`.
    HeightUnit { Raw => "raw", "raw", Centimeters => "cm", "cm", Meters => "m", "m", Feet => "ft", "ft" }
}
unit_enum! {
    /// Unit of `gyroADC[0..2]`.
    RotationUnit { Raw => "raw", "raw", DegreesPerSecond => "deg/s", "deg/s", RadiansPerSecond => "rad/s", "rad/s" }
}
unit_enum! {
    /// Unit of `accSmooth[0..2]`.
    AccelerationUnit { Raw => "raw", "raw", Gs => "g", "g", MetersPerSecondSquared => "m/s2", "m/s/s" }
}

/// Output unit choices for one export.
///
/// `Default` matches the historic output: volts, amps, decoded flags,
/// microseconds, raw gyro and accelerometer values, and `BaroAlt` untouched
/// (`height` is `None`). An explicit `Some(HeightUnit::Centimeters)` adds the
/// `(cm)` header suffix that upstream prints by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitOptions {
    /// `--unit-vbat`
    pub vbat: VbatUnit,
    /// `--unit-amperage`
    pub amperage: AmperageUnit,
    /// `--unit-flags`
    pub flags: FlagsUnit,
    /// `--unit-frame-time`
    pub frame_time: FrameTimeUnit,
    /// `--unit-height`; `None` keeps the plain `BaroAlt` column.
    pub height: Option<HeightUnit>,
    /// `--unit-rotation`
    pub rotation: RotationUnit,
    /// `--unit-acceleration`
    pub acceleration: AccelerationUnit,
}

impl Default for UnitOptions {
    fn default() -> Self {
        Self {
            vbat: VbatUnit::Volts,
            amperage: AmperageUnit::Amps,
            flags: FlagsUnit::Flags,
            frame_time: FrameTimeUnit::Microseconds,
            height: None,
            rotation: RotationUnit::Raw,
            acceleration: AccelerationUnit::Raw,
        }
    }
}

/// Parse a finite altitude offset in meters (`--alt-offset`).
///
/// Upstream uses `atof`, which turns garbage into 0. This rejects text that is
/// not a finite number instead.
pub fn parse_alt_offset(text: &str) -> Result<f64, String> {
    match text.trim().parse::<f64>() {
        Ok(value) if value.is_finite() => Ok(value),
        _ => Err(format!(
            "invalid altitude offset '{text}', expected a finite number of meters"
        )),
    }
}

/// Value of the first `H <key>:<value>` header line whose key is in `keys`.
fn header_value<'a>(header: &'a BBLHeader, keys: &[&str]) -> Option<&'a str> {
    header.all_headers.iter().find_map(|line| {
        let (key, value) = line.strip_prefix("H ")?.split_once(':')?;
        keys.contains(&key.trim()).then(|| value.trim())
    })
}

/// Raw accelerometer counts for 1 g, from the `acc_1G` header.
pub fn acc_1g(header: &BBLHeader) -> Option<i32> {
    header
        .sysconfig
        .get(HEADER_ACC_1G)
        .copied()
        .filter(|v| *v != 0)
}

/// Gyro scale in radians per microsecond, from the `gyro_scale` header.
///
/// The header holds the `f32` bits as hex. Only firmware type `Baseflight`
/// already logs radians per microsecond; every other firmware logs degrees per
/// second and is converted, as `parser.c` does for every firmware type other
/// than Baseflight.
pub fn gyro_scale_radians_per_microsecond(header: &BBLHeader) -> Option<f32> {
    let hex = header_value(header, &[HEADER_GYRO_SCALE, HEADER_GYRO_SCALE_DOTTED])?;
    let digits = hex
        .strip_prefix("0x")
        .or_else(|| hex.strip_prefix("0X"))
        .unwrap_or(hex);
    let scale = f32::from_bits(u32::from_str_radix(digits, 16).ok()?);
    if header_value(header, &[HEADER_FIRMWARE_TYPE]) == Some(NATIVE_RADIANS_FIRMWARE_TYPE) {
        Some(scale)
    } else {
        Some((scale as f64 * (std::f64::consts::PI / 180.0) * 0.000001) as f32)
    }
}

/// Convert a raw gyro count to the requested unit.
pub fn gyro_to_unit(raw: i32, scale: f32, unit: RotationUnit) -> f64 {
    let radians = scale as f64 * MICROSECONDS_PER_SECOND * raw as f64;
    match unit {
        RotationUnit::RadiansPerSecond => radians,
        _ => radians * (180.0 / std::f64::consts::PI),
    }
}

/// Convert a raw accelerometer count to the requested unit.
pub fn acceleration_to_unit(raw: i32, acc_1g: i32, unit: AccelerationUnit) -> f64 {
    let gs = raw as f64 / acc_1g as f64;
    match unit {
        AccelerationUnit::MetersPerSecondSquared => gs * ACCELERATION_DUE_TO_GRAVITY,
        _ => gs,
    }
}

/// Convert a raw `BaroAlt` value (centimeters) to the requested unit.
pub fn height_to_unit(raw: i32, unit: HeightUnit) -> f64 {
    let meters = raw as f64 / 100.0;
    match unit {
        HeightUnit::Feet => meters * FEET_PER_METER,
        _ => meters,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_parse_case_insensitive() {
        assert_eq!("MV".parse::<VbatUnit>(), Ok(VbatUnit::MilliVolts));
        assert_eq!(
            "Deg/S".parse::<RotationUnit>(),
            Ok(RotationUnit::DegreesPerSecond)
        );
        assert_eq!(
            "M/S2".parse::<AccelerationUnit>(),
            Ok(AccelerationUnit::MetersPerSecondSquared)
        );
        assert_eq!("FT".parse::<HeightUnit>(), Ok(HeightUnit::Feet));
    }

    #[test]
    fn wrong_category_and_garbage_rejected() {
        assert!("ft".parse::<VbatUnit>().is_err());
        assert!("V".parse::<AmperageUnit>().is_err());
        assert!("".parse::<FlagsUnit>().is_err());
        assert!("us ".parse::<FrameTimeUnit>().is_err());
        assert!("kph".parse::<HeightUnit>().is_err());
        assert!("mV; drop".parse::<VbatUnit>().is_err());
    }

    #[test]
    fn labels() {
        assert_eq!(VbatUnit::Raw.label(), None);
        assert_eq!(
            AccelerationUnit::MetersPerSecondSquared.label(),
            Some("m/s/s")
        );
        assert_eq!(FrameTimeUnit::Seconds.label(), Some("s"));
    }

    #[test]
    fn alt_offset_rejects_non_finite() {
        assert_eq!(parse_alt_offset("-12.5"), Ok(-12.5));
        assert!(parse_alt_offset("nan").is_err());
        assert!(parse_alt_offset("inf").is_err());
        assert!(parse_alt_offset("abc").is_err());
        assert!(parse_alt_offset("").is_err());
    }

    #[test]
    fn conversions_follow_upstream() {
        assert_eq!(height_to_unit(12345, HeightUnit::Meters), 123.45);
        assert!((height_to_unit(10000, HeightUnit::Feet) - 328.084).abs() < 1e-9);
        assert_eq!(acceleration_to_unit(4096, 2048, AccelerationUnit::Gs), 2.0);
        assert!(
            (acceleration_to_unit(2048, 2048, AccelerationUnit::MetersPerSecondSquared) - 9.80665)
                .abs()
                < 1e-12
        );
        // gyro_scale 1.0 deg/s per count => rad/s per count = pi/180
        let scale = (1.0f64 * (std::f64::consts::PI / 180.0) * 0.000001) as f32;
        assert!((gyro_to_unit(180, scale, RotationUnit::DegreesPerSecond) - 180.0).abs() < 1e-3);
        assert!(
            (gyro_to_unit(180, scale, RotationUnit::RadiansPerSecond) - std::f64::consts::PI).abs()
                < 1e-3
        );
    }

    #[test]
    fn gyro_scale_header_parsing() {
        let mut header = BBLHeader::default();
        header
            .all_headers
            .push("H Firmware type:Betaflight".to_string());
        header
            .all_headers
            .push("H gyro_scale:0x3f800000".to_string());
        let scale = gyro_scale_radians_per_microsecond(&header).unwrap();
        assert!((scale as f64 - std::f64::consts::PI / 180.0 * 1e-6).abs() < 1e-12);
        header.all_headers = vec![
            "H Firmware type:Baseflight".to_string(),
            "H gyro_scale:0x3f800000".to_string(),
        ];
        assert_eq!(gyro_scale_radians_per_microsecond(&header), Some(1.0));
        header.all_headers = vec!["H gyro_scale:zz".to_string()];
        assert_eq!(gyro_scale_radians_per_microsecond(&header), None);
    }

    #[test]
    fn default_is_historic_output() {
        let d = UnitOptions::default();
        assert_eq!(d.vbat, VbatUnit::Volts);
        assert_eq!(d.amperage, AmperageUnit::Amps);
        assert_eq!(d.flags, FlagsUnit::Flags);
        assert_eq!(d.frame_time, FrameTimeUnit::Microseconds);
        assert_eq!(d.height, None);
    }
}
