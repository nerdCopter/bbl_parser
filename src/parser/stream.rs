use crate::parser::helpers::{
    sign_extend_14bit, sign_extend_16bit, sign_extend_24bit, sign_extend_2bit, sign_extend_4bit,
    sign_extend_6bit, sign_extend_8bit,
};
use anyhow::Result;

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
            Err(anyhow::anyhow!("EOF"))
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
    /// When value_count is 1, reads single signed VB without header byte.
    /// Otherwise reads header byte followed by up to 8 values based on header bits.
    #[allow(clippy::needless_range_loop)]
    pub fn read_tag8_8svb(&mut self, values: &mut [i32]) -> Result<()> {
        // Fixed 8-value version for internal use
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

    /// Read Tag8_8SVB encoding with variable count
    /// When value_count is 1, reads single signed VB without header byte.
    /// Otherwise reads header byte followed by up to value_count values based on header bits.
    #[allow(clippy::needless_range_loop)]
    pub fn read_tag8_8svb_counted(&mut self, values: &mut [i32], value_count: usize) -> Result<()> {
        if value_count == 1 {
            values[0] = self.read_signed_vb()?;
        } else {
            let mut header = self.read_byte()?;
            for i in 0..8.min(value_count) {
                values[i] = if header & 0x01 != 0 {
                    self.read_signed_vb()?
                } else {
                    0
                };
                header >>= 1;
            }
        }
        Ok(())
    }

    /// Read negative 14-bit encoding (sign-magnitude format)
    /// Reads an unsigned variable byte and interprets it as a 14-bit sign-magnitude value.
    /// Bit 13 is the sign bit, bits 0-12 are the magnitude.
    /// Returns the negated value to match blackbox_decode behavior.
    pub fn read_neg_14bit(&mut self) -> Result<i32> {
        let unsigned = self.read_unsigned_vb()? as u16;
        Ok(-sign_extend_14bit(unsigned))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::helpers::sign_extend_14bit;

    #[test]
    fn test_sign_extend_14bit_sign_magnitude_positive() {
        // Positive values have bit 13 = 0 (sign bit clear)
        assert_eq!(sign_extend_14bit(0x0000), 0); // 0
        assert_eq!(sign_extend_14bit(0x0001), 1); // 1
        assert_eq!(sign_extend_14bit(0x1FFF), 0x1FFF); // 8191 (max positive magnitude)
    }

    #[test]
    fn test_sign_extend_14bit_sign_magnitude_negative() {
        // Negative values have bit 13 = 1 (sign bit set), magnitude in bits 0-12
        // 0x2000 = bit 13 set, magnitude 0 -> returns -0 = 0 (actually negative zero)
        assert_eq!(sign_extend_14bit(0x2000), 0); // -0
                                                  // 0x2001 = bit 13 set, magnitude 1 -> returns -1
        assert_eq!(sign_extend_14bit(0x2001), -1);
        // 0x3FFF = bit 13 set, magnitude 0x1FFF (8191) -> returns -8191
        assert_eq!(sign_extend_14bit(0x3FFF), -8191);
    }

    #[test]
    fn test_read_neg_14bit_positive() {
        // Test reading positive 14-bit value from variable byte encoding
        // VB encoding of 100 is [100] (single byte since 100 < 128)
        // sign_extend_14bit_sign_magnitude(100) = 100 (bit 13 not set)
        // read_neg_14bit returns -100 (negation)
        let data = vec![100u8];
        let mut stream = BBLDataStream::new(&data);
        // The function returns the negation: -sign_extend_14bit_sign_magnitude(100) = -100
        assert_eq!(stream.read_neg_14bit().unwrap(), -100);
    }

    #[test]
    fn test_read_neg_14bit_negative() {
        // Test reading value with sign bit set
        // VB encode 0x2001 = 8193: 0x81 (1 + continuation), 0x40 (64, final) = 1 + 64*128 = 8193
        let data = vec![0x81, 0x40u8];
        let mut stream = BBLDataStream::new(&data);
        // 0x2001: bit 13 set, magnitude = 1, sign_extend returns -1
        // read_neg_14bit returns -(-1) = 1
        assert_eq!(stream.read_neg_14bit().unwrap(), 1);
    }

    #[test]
    fn test_read_neg_14bit_boundary() {
        // Test boundary values
        // Value 0: VB = [0], sign_extend_14bit(0) = 0, read_neg_14bit returns -0 = 0
        let data = vec![0u8];
        let mut stream = BBLDataStream::new(&data);
        assert_eq!(stream.read_neg_14bit().unwrap(), 0);

        // Max magnitude positive (no sign): 0x1FFF = 8191
        // VB encode 0x1FFF: 0xFF, 0x3F = 127 + 63*128 = 8191
        // sign_extend returns 8191, read_neg_14bit returns -8191
        let data = vec![0xFF, 0x3Fu8];
        let mut stream = BBLDataStream::new(&data);
        assert_eq!(stream.read_neg_14bit().unwrap(), -8191);
    }

    #[test]
    fn test_read_neg_14bit_with_sign_bit() {
        // When sign bit (bit 13) is set, sign_extend returns negative, then we negate again
        // 0x2001 encodes as VB: 0x81, 0x40 = 1 + 64*128 = 8193
        // sign_extend_14bit_sign_magnitude(0x2001) = -1 (sign bit set, magnitude 1)
        // read_neg_14bit returns -(-1) = 1
        let data = vec![0x81, 0x40u8];
        let mut stream = BBLDataStream::new(&data);
        assert_eq!(stream.read_neg_14bit().unwrap(), 1);
    }
}
