use crate::error::{BBLError, Result};

/// BBL data stream for reading binary data
pub struct BBLDataStream<'a> {
    data: &'a [u8],
    pub pos: usize,
    end: usize,
    pub eof: bool,
}

impl<'a> BBLDataStream<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            end: data.len(),
            eof: false,
        }
    }

    pub fn set_position(&mut self, pos: usize) {
        self.pos = pos;
        self.eof = pos >= self.end;
    }

    pub fn read_byte(&mut self) -> Result<u8> {
        if self.pos < self.end {
            let byte = self.data[self.pos];
            self.pos += 1;
            Ok(byte)
        } else {
            self.eof = true;
            Err(BBLError::UnexpectedEof)
        }
    }

    pub fn read_char(&mut self) -> Result<char> {
        Ok(self.read_byte()? as char)
    }

    /// Read unsigned variable byte - exact replica of JavaScript implementation
    pub fn read_unsigned_vb(&mut self) -> Result<u32> {
        let mut result = 0u32;
        let mut shift = 0;

        // 5 bytes is enough to encode 32-bit unsigned quantities
        for _ in 0..5 {
            let b = match self.read_byte() {
                Ok(byte) => byte,
                Err(_) => return Ok(0),
            };

            result |= ((b & !0x80) as u32) << shift;

            // Final byte?
            if b < 128 {
                return Ok(result);
            }

            shift += 7;
        }

        // This VB-encoded int is too long!
        Ok(0)
    }

    /// Read signed variable byte - exact replica of JavaScript implementation
    pub fn read_signed_vb(&mut self) -> Result<i32> {
        let unsigned = self.read_unsigned_vb()?;

        // Apply ZigZag decoding to recover the signed value
        Ok(((unsigned >> 1) as i32) ^ -((unsigned & 1) as i32))
    }

    /// Read Tag8_4S16 encoding - exact replica of JavaScript implementation
    #[allow(clippy::needless_range_loop)]
    pub fn read_tag8_4s16_v2(&mut self, values: &mut [i32]) -> Result<()> {
        let selector = self.read_byte()?;
        let mut nibble_index = 0;
        let mut buffer = 0u8;

        for i in 0..4 {
            let field_type = (selector >> (i * 2)) & 0x03;

            match field_type {
                0 => values[i] = 0, // FIELD_ZERO
                1 => {
                    // FIELD_4BIT
                    if nibble_index == 0 {
                        buffer = self.read_byte()?;
                        values[i] = sign_extend_4bit(buffer >> 4);
                        nibble_index = 1;
                    } else {
                        values[i] = sign_extend_4bit(buffer & 0x0f);
                        nibble_index = 0;
                    }
                }
                2 => {
                    // FIELD_8BIT
                    if nibble_index == 0 {
                        values[i] = sign_extend_8bit(self.read_byte()?);
                    } else {
                        let mut char1 = (buffer & 0x0f) << 4;
                        buffer = self.read_byte()?;
                        char1 |= buffer >> 4;
                        values[i] = sign_extend_8bit(char1);
                    }
                }
                3 => {
                    // FIELD_16BIT
                    if nibble_index == 0 {
                        let char1 = self.read_byte()?;
                        let char2 = self.read_byte()?;
                        values[i] = sign_extend_16bit(((char1 as u16) << 8) | (char2 as u16));
                    } else {
                        let char1 = self.read_byte()?;
                        let char2 = self.read_byte()?;
                        values[i] = sign_extend_16bit(
                            (((buffer & 0x0f) as u16) << 12)
                                | ((char1 as u16) << 4)
                                | ((char2 as u16) >> 4),
                        );
                        buffer = char2;
                    }
                }
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    /// Read Tag2_3S32 encoding - exact replica of JavaScript implementation
    pub fn read_tag2_3s32(&mut self, values: &mut [i32]) -> Result<()> {
        let lead_byte = self.read_byte()?;

        match lead_byte >> 6 {
            0 => {
                // 2-bit fields
                values[0] = sign_extend_2bit((lead_byte >> 4) & 0x03);
                values[1] = sign_extend_2bit((lead_byte >> 2) & 0x03);
                values[2] = sign_extend_2bit(lead_byte & 0x03);
            }
            1 => {
                // 4-bit fields
                values[0] = sign_extend_4bit(lead_byte & 0x0f);
                let second_byte = self.read_byte()?;
                values[1] = sign_extend_4bit(second_byte >> 4);
                values[2] = sign_extend_4bit(second_byte & 0x0f);
            }
            2 => {
                // 6-bit fields
                values[0] = sign_extend_6bit(lead_byte & 0x3f);
                let byte2 = self.read_byte()?;
                values[1] = sign_extend_6bit(byte2 & 0x3f);
                let byte3 = self.read_byte()?;
                values[2] = sign_extend_6bit(byte3 & 0x3f);
            }
            3 => {
                // 8, 16 or 24 bit fields
                let mut selector = lead_byte;
                #[allow(clippy::needless_range_loop)]
                for i in 0..3 {
                    match selector & 0x03 {
                        0 => {
                            // 8-bit
                            let byte1 = self.read_byte()?;
                            values[i] = sign_extend_8bit(byte1);
                        }
                        1 => {
                            // 16-bit
                            let byte1 = self.read_byte()?;
                            let byte2 = self.read_byte()?;
                            values[i] = sign_extend_16bit((byte1 as u16) | ((byte2 as u16) << 8));
                        }
                        2 => {
                            // 24-bit
                            let byte1 = self.read_byte()?;
                            let byte2 = self.read_byte()?;
                            let byte3 = self.read_byte()?;
                            values[i] = sign_extend_24bit(
                                (byte1 as u32) | ((byte2 as u32) << 8) | ((byte3 as u32) << 16),
                            );
                        }
                        3 => {
                            // 32-bit
                            let byte1 = self.read_byte()?;
                            let byte2 = self.read_byte()?;
                            let byte3 = self.read_byte()?;
                            let byte4 = self.read_byte()?;
                            values[i] = (byte1 as i32)
                                | ((byte2 as i32) << 8)
                                | ((byte3 as i32) << 16)
                                | ((byte4 as i32) << 24);
                        }
                        _ => unreachable!(),
                    }
                    selector >>= 2;
                }
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    /// Read Tag8_8SVB encoding - exact replica of JavaScript implementation
    #[allow(clippy::needless_range_loop)]
    pub fn read_tag8_8svb(&mut self, values: &mut [i32]) -> Result<()> {
        let selector = self.read_byte()?;

        for i in 0..8 {
            if (selector & (1 << i)) != 0 {
                values[i] = self.read_signed_vb()?;
            } else {
                values[i] = 0;
            }
        }

        Ok(())
    }

    /// Read negative 14-bit encoding
    /// Reads an unsigned variable byte and interprets it as a 14-bit two's complement signed value.
    /// The value is masked to 14 bits (0x3FFF), with bit 13 serving as the sign bit.
    /// Negative values (sign bit set) are sign-extended to i32.
    pub fn read_neg_14bit(&mut self) -> Result<i32> {
        let unsigned = self.read_unsigned_vb()?;

        // Mask to 14 bits and perform sign-extension
        // If bit 13 is set, the value is negative and needs sign-extension
        let masked = (unsigned & 0x3FFF) as i32;
        Ok(sign_extend_14bit(masked as u16))
    }
}

// Sign extension helper functions - exact replicas of JavaScript implementation
fn sign_extend_2bit(value: u8) -> i32 {
    if (value & 0x02) != 0 {
        (value as i32) | !0x03
    } else {
        value as i32
    }
}

fn sign_extend_4bit(value: u8) -> i32 {
    if (value & 0x08) != 0 {
        (value as i32) | !0x0f
    } else {
        value as i32
    }
}

fn sign_extend_6bit(value: u8) -> i32 {
    if (value & 0x20) != 0 {
        (value as i32) | !0x3f
    } else {
        value as i32
    }
}

fn sign_extend_8bit(value: u8) -> i32 {
    value as i8 as i32
}

fn sign_extend_14bit(value: u16) -> i32 {
    if (value & 0x2000) != 0 {
        (value as i32) | !0x3fff
    } else {
        (value & 0x3fff) as i32
    }
}

fn sign_extend_16bit(value: u16) -> i32 {
    value as i16 as i32
}

fn sign_extend_24bit(value: u32) -> i32 {
    if (value & 0x800000) != 0 {
        (value as i32) | !0xffffff
    } else {
        value as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_extend_14bit_positive() {
        // Positive values have bit 13 = 0
        assert_eq!(sign_extend_14bit(0x0000), 0); // 0
        assert_eq!(sign_extend_14bit(0x0001), 1); // 1
        assert_eq!(sign_extend_14bit(0x1FFF), 0x1FFF); // 8191 (max positive)
    }

    #[test]
    fn test_sign_extend_14bit_negative() {
        // Negative values have bit 13 = 1, sign extended to all upper bits
        assert_eq!(sign_extend_14bit(0x2000), -8192); // -8192 (min negative)
        assert_eq!(sign_extend_14bit(0x3FFF), -1); // -1
        assert_eq!(sign_extend_14bit(0x2001), -8191); // -8191
    }

    #[test]
    fn test_read_neg_14bit_positive() {
        // Test reading positive 14-bit value from variable byte encoding
        // VB encoding of 100 is [100] (single byte since 100 < 128)
        let data = vec![100u8];
        let mut stream = BBLDataStream::new(&data);
        assert_eq!(stream.read_neg_14bit().unwrap(), 100);
    }

    #[test]
    fn test_read_neg_14bit_negative() {
        // Test reading negative 14-bit value
        // 14-bit value -1 (0x3FFF in two's complement)
        // VB encode 0x3FFF: 0x3FFF = 16383
        // 16383 in VB: 0xFF (127 + continuation), 0x7F (127, final) = 127 + 127*128 = 16383
        let data = vec![0xFF, 0x7Fu8];
        let mut stream = BBLDataStream::new(&data);
        assert_eq!(stream.read_neg_14bit().unwrap(), -1);
    }

    #[test]
    fn test_read_neg_14bit_boundary() {
        // Test boundary values
        // Max positive: 0x1FFF (8191)
        // VB encode 0x1FFF: 0xFF (127 + continuation), 0x3F (63, final) = 127 + 63*128 = 8191
        let data = vec![0xFF, 0x3Fu8];
        let mut stream = BBLDataStream::new(&data);
        assert_eq!(stream.read_neg_14bit().unwrap(), 8191);

        // Min negative: 0x2000 (-8192)
        // VB encode 0x2000: 0x80 (0 + continuation), 0x20 (32, final) = 0 + 32*128 = 4096
        // But 0x2000 & 0x3FFF = 0x2000, and bit 13 is set, so it's negative
        // Actually we need the full 14-bit value 0x2000 = 8192
        // In VB that's: 0x80, 0x40 = 0 + 64*128 = 8192
        let data = vec![0x80, 0x40u8];
        let mut stream = BBLDataStream::new(&data);
        assert_eq!(stream.read_neg_14bit().unwrap(), -8192);
    }

    #[test]
    fn test_read_neg_14bit_masks_14_bits() {
        // Verify that only lower 14 bits are used even if VB encodes more
        // If VB reads a value > 0x3FFF, only lower 14 bits are used
        // Encode 0xFFFFF (large value), which masks to 0x3FFF = -1
        // VB encode 0xFFFFF: 0xFF, 0xFF, 0x7F = 127 + 127*128 + 127*128^2
        let data = vec![0xFF, 0xFF, 0x7Fu8];
        let mut stream = BBLDataStream::new(&data);
        let result = stream.read_neg_14bit().unwrap();
        // 0xFFFFF & 0x3FFF = 0x3FFF, which is -1 in 14-bit two's complement
        assert_eq!(result, -1);
    }
}
