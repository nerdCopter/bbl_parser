//! Helper functions for BBL parsing
//!
//! This module provides sign extension functions used for decoding various
//! fixed-width signed values from the blackbox binary format.

/// Sign-extend a 2-bit value to i32
pub fn sign_extend_2bit(value: u8) -> i32 {
    let val = value as i32;
    if (val & 0x02) != 0 {
        val | !0x03
    } else {
        val & 0x03
    }
}

/// Sign-extend a 4-bit value to i32
pub fn sign_extend_4bit(value: u8) -> i32 {
    let val = value as i32;
    if (val & 0x08) != 0 {
        val | !0x0f
    } else {
        val & 0x0f
    }
}

/// Sign-extend a 6-bit value to i32
pub fn sign_extend_6bit(value: u8) -> i32 {
    let val = value as i32;
    if (val & 0x20) != 0 {
        val | !0x3f
    } else {
        val & 0x3f
    }
}

/// Sign-extend an 8-bit value to i32
pub fn sign_extend_8bit(value: u8) -> i32 {
    value as i8 as i32
}

/// Sign-extend a 16-bit value to i32
pub fn sign_extend_16bit(value: u16) -> i32 {
    value as i16 as i32
}

/// Sign-extend a 24-bit value to i32
pub fn sign_extend_24bit(value: u32) -> i32 {
    if (value & 0x800000) != 0 {
        (value | 0xff000000) as i32
    } else {
        (value & 0x7fffff) as i32
    }
}

/// Sign-extend a 14-bit value to i32 (sign-magnitude format)
/// Bit 13 indicates sign, bits 0-12 are the magnitude.
/// Returns negative value if sign bit is set.
pub fn sign_extend_14bit(value: u16) -> i32 {
    if (value & 0x2000) != 0 {
        -((value & 0x1fff) as i32)
    } else {
        (value & 0x1fff) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_extend_2bit() {
        assert_eq!(sign_extend_2bit(0), 0);
        assert_eq!(sign_extend_2bit(1), 1);
        assert_eq!(sign_extend_2bit(2), -2);
        assert_eq!(sign_extend_2bit(3), -1);
    }

    #[test]
    fn test_sign_extend_4bit() {
        assert_eq!(sign_extend_4bit(0), 0);
        assert_eq!(sign_extend_4bit(7), 7);
        assert_eq!(sign_extend_4bit(8), -8);
        assert_eq!(sign_extend_4bit(15), -1);
    }

    #[test]
    fn test_sign_extend_6bit() {
        assert_eq!(sign_extend_6bit(0), 0);
        assert_eq!(sign_extend_6bit(31), 31);
        assert_eq!(sign_extend_6bit(32), -32);
        assert_eq!(sign_extend_6bit(63), -1);
    }

    #[test]
    fn test_sign_extend_8bit() {
        assert_eq!(sign_extend_8bit(0), 0);
        assert_eq!(sign_extend_8bit(127), 127);
        assert_eq!(sign_extend_8bit(128), -128);
        assert_eq!(sign_extend_8bit(255), -1);
    }

    #[test]
    fn test_sign_extend_16bit() {
        assert_eq!(sign_extend_16bit(0), 0);
        assert_eq!(sign_extend_16bit(32767), 32767);
        assert_eq!(sign_extend_16bit(32768), -32768);
        assert_eq!(sign_extend_16bit(65535), -1);
    }

    #[test]
    fn test_sign_extend_24bit() {
        assert_eq!(sign_extend_24bit(0), 0);
        assert_eq!(sign_extend_24bit(0x7FFFFF), 0x7FFFFF);
        assert_eq!(sign_extend_24bit(0x800000), -8388608);
        assert_eq!(sign_extend_24bit(0xFFFFFF), -1);
    }

    #[test]
    fn test_sign_extend_14bit() {
        // Positive values (bit 13 clear)
        assert_eq!(sign_extend_14bit(0), 0);
        assert_eq!(sign_extend_14bit(1), 1);
        assert_eq!(sign_extend_14bit(0x1FFF), 0x1FFF); // 8191

        // Negative values (bit 13 set)
        assert_eq!(sign_extend_14bit(0x2000), 0); // -0
        assert_eq!(sign_extend_14bit(0x2001), -1);
        assert_eq!(sign_extend_14bit(0x3FFF), -8191);
    }
}
