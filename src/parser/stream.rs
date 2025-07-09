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
    pub fn read_tag8_4s16_v2(&mut self, values: &mut [i32]) -> Result<()> {
        let selector = self.read_byte()?;
        let mut nibble_index = 0;
        let mut buffer = 0u8;

        for i in 0..4 {
            let field_type = (selector >> (i * 2)) & 0x03;

            match field_type {
                0 => values[i] = 0, // FIELD_ZERO
                1 => { // FIELD_4BIT
                    if nibble_index == 0 {
                        buffer = self.read_byte()?;
                        values[i] = sign_extend_4bit(buffer >> 4);
                        nibble_index = 1;
                    } else {
                        values[i] = sign_extend_4bit(buffer & 0x0f);
                        nibble_index = 0;
                    }
                }
                2 => { // FIELD_8BIT
                    if nibble_index == 0 {
                        values[i] = sign_extend_8bit(self.read_byte()?);
                    } else {
                        let mut char1 = ((buffer & 0x0f) << 4) as u8;
                        buffer = self.read_byte()?;
                        char1 |= buffer >> 4;
                        values[i] = sign_extend_8bit(char1);
                    }
                }
                3 => { // FIELD_16BIT
                    if nibble_index == 0 {
                        let char1 = self.read_byte()?;
                        let char2 = self.read_byte()?;
                        values[i] = sign_extend_16bit(((char1 as u16) << 8) | (char2 as u16));
                    } else {
                        let char1 = self.read_byte()?;
                        let char2 = self.read_byte()?;
                        values[i] = sign_extend_16bit(
                            (((buffer & 0x0f) as u16) << 12) | 
                            ((char1 as u16) << 4) | 
                            ((char2 as u16) >> 4)
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
            0 => { // 2-bit fields
                values[0] = sign_extend_2bit((lead_byte >> 4) & 0x03);
                values[1] = sign_extend_2bit((lead_byte >> 2) & 0x03);
                values[2] = sign_extend_2bit(lead_byte & 0x03);
            }
            1 => { // 4-bit fields
                values[0] = sign_extend_4bit(lead_byte & 0x0f);
                let second_byte = self.read_byte()?;
                values[1] = sign_extend_4bit(second_byte >> 4);
                values[2] = sign_extend_4bit(second_byte & 0x0f);
            }
            2 => { // 6-bit fields
                values[0] = sign_extend_6bit(lead_byte & 0x3f);
                let byte2 = self.read_byte()?;
                values[1] = sign_extend_6bit(byte2 & 0x3f);
                let byte3 = self.read_byte()?;
                values[2] = sign_extend_6bit(byte3 & 0x3f);
            }
            3 => { // 8, 16 or 24 bit fields
                let mut selector = lead_byte;
                for i in 0..3 {
                    match selector & 0x03 {
                        0 => { // 8-bit
                            let byte1 = self.read_byte()?;
                            values[i] = sign_extend_8bit(byte1);
                        }
                        1 => { // 16-bit
                            let byte1 = self.read_byte()?;
                            let byte2 = self.read_byte()?;
                            values[i] = sign_extend_16bit((byte1 as u16) | ((byte2 as u16) << 8));
                        }
                        2 => { // 24-bit
                            let byte1 = self.read_byte()?;
                            let byte2 = self.read_byte()?;
                            let byte3 = self.read_byte()?;
                            values[i] = sign_extend_24bit(
                                (byte1 as u32) | ((byte2 as u32) << 8) | ((byte3 as u32) << 16)
                            );
                        }
                        3 => { // 32-bit
                            let byte1 = self.read_byte()?;
                            let byte2 = self.read_byte()?;
                            let byte3 = self.read_byte()?;
                            let byte4 = self.read_byte()?;
                            values[i] = (byte1 as i32) | ((byte2 as i32) << 8) | ((byte3 as i32) << 16) | ((byte4 as i32) << 24);
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

    /// Read negative 14-bit encoding - exact replica of JavaScript implementation
    pub fn read_neg_14bit(&mut self) -> Result<i32> {
        let byte1 = self.read_byte()? as u16;
        let byte2 = self.read_byte()? as u16;
        
        let unsigned_value = (byte1 << 6) | (byte2 >> 2);
        
        // Convert to signed 14-bit value and make it negative
        let signed_value = if unsigned_value > 8191 {
            (unsigned_value as i32) - 16384
        } else {
            unsigned_value as i32
        };
        
        Ok(-signed_value)
    }
    
    /// Skip ahead in the stream until finding a valid frame marker
    /// This is a safe implementation that searches for valid frame markers (I, P, E, G, H, S)
    /// without causing stream corruption
    pub fn skip_to_next_marker(&mut self) -> Result<char> {
        let mut marker_candidates = 0;
        
        // Search through the stream for a valid frame marker
        while !self.eof {
            let byte = match self.read_byte() {
                Ok(b) => b,
                Err(_) => return Err(BBLError::UnexpectedEof),
            };
            
            let c = byte as char;
            
            // Valid frame markers are ASCII letters: I, P, E, G, H, S
            if (c == 'I' || c == 'P' || c == 'E' || c == 'G' || c == 'H' || c == 'S') {
                // Found a potential marker - check if it's valid by ensuring the next byte can be read
                // This is a simple heuristic to reduce false positives
                if self.pos < self.end {
                    marker_candidates += 1;
                    
                    // Return the marker after confirming it's valid
                    // Backtrack position by 1 so the next read will get this marker
                    self.pos -= 1;
                    return Ok(c);
                }
            }
            
            // Safety limit - don't scan too far
            if marker_candidates > 1000 {
                return Err(BBLError::InvalidData("Too many false marker candidates".into()));
            }
        }
        
        Err(BBLError::UnexpectedEof)
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
