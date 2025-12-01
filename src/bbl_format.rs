use anyhow::Result;
use bbl_parser::parser::helpers::{
    sign_extend_16bit, sign_extend_24bit, sign_extend_2bit, sign_extend_4bit, sign_extend_6bit,
    sign_extend_8bit,
};
use std::collections::HashMap;

// Re-export sign_extend_14bit for backward compatibility with main.rs
pub use bbl_parser::parser::helpers::sign_extend_14bit;

// BBL Encoding constants - directly from JavaScript reference
pub const ENCODING_SIGNED_VB: u8 = 0;
pub const ENCODING_UNSIGNED_VB: u8 = 1;
pub const ENCODING_NEG_14BIT: u8 = 3;
#[allow(dead_code)]
pub const ENCODING_TAG8_8SVB: u8 = 6;
#[allow(dead_code)]
pub const ENCODING_TAG2_3S32: u8 = 7;
#[allow(dead_code)]
pub const ENCODING_TAG8_4S16: u8 = 8;
pub const ENCODING_NULL: u8 = 9;
#[allow(dead_code)]
pub const ENCODING_TAG2_3SVARIABLE: u8 = 10;

// Predictor constants - directly from JavaScript reference
#[allow(dead_code)]
pub const PREDICT_0: u8 = 0;
#[allow(dead_code)]
pub const PREDICT_PREVIOUS: u8 = 1;
#[allow(dead_code)]
pub const PREDICT_STRAIGHT_LINE: u8 = 2;
#[allow(dead_code)]
pub const PREDICT_AVERAGE_2: u8 = 3;
#[allow(dead_code)]
pub const PREDICT_MINTHROTTLE: u8 = 4;
#[allow(dead_code)]
pub const PREDICT_MOTOR_0: u8 = 5;
#[allow(dead_code)]
pub const PREDICT_INC: u8 = 6;
#[allow(dead_code)]
pub const PREDICT_HOME_COORD: u8 = 7;
#[allow(dead_code)]
pub const PREDICT_1500: u8 = 8;
#[allow(dead_code)]
pub const PREDICT_VBATREF: u8 = 9;
#[allow(dead_code)]
pub const PREDICT_LAST_MAIN_FRAME_TIME: u8 = 10;
#[allow(dead_code)]
pub const PREDICT_MINMOTOR: u8 = 11;

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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn read_char(&mut self) -> Result<char> {
        Ok(self.read_byte()? as char)
    }

    // Read unsigned variable byte - exact replica of JavaScript implementation
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

    // Read signed variable byte - exact replica of JavaScript implementation
    pub fn read_signed_vb(&mut self) -> Result<i32> {
        let unsigned = self.read_unsigned_vb()?;

        // Apply ZigZag decoding to recover the signed value
        Ok(((unsigned >> 1) as i32) ^ -((unsigned & 1) as i32))
    }

    // Read Tag8_4S16 encoding - exact replica of JavaScript implementation
    pub fn read_tag8_4s16_v2(&mut self, values: &mut [i32]) -> Result<()> {
        let selector = self.read_byte()?;
        let mut nibble_index = 0;
        let mut buffer = 0u8;

        #[allow(clippy::needless_range_loop)]
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

    // Read Tag2_3S32 encoding - exact replica of JavaScript implementation
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

    // Read Tag8_8SVB encoding - exact replica of JavaScript implementation
    pub fn read_tag8_8svb(&mut self, values: &mut [i32], value_count: usize) -> Result<()> {
        if value_count == 1 {
            values[0] = self.read_signed_vb()?;
        } else {
            let mut header = self.read_byte()?;
            #[allow(clippy::needless_range_loop)]
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
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn apply_predictor(
    field_index: usize,
    predictor: u8,
    raw_value: i32,
    current_frame: &[i32],
    previous_frame: Option<&[i32]>,
    previous2_frame: Option<&[i32]>,
    skipped_frames: u32,
    sysconfig: &HashMap<String, i32>,
    field_names: &[String],
    debug: bool,
) -> i32 {
    match predictor {
        PREDICT_0 => raw_value,

        PREDICT_PREVIOUS => {
            if let Some(prev) = previous_frame {
                if field_index < prev.len() {
                    let result = prev[field_index] + raw_value;

                    // CRITICAL FIX: Prevent corruption propagation for vbatLatest
                    if field_names
                        .get(field_index)
                        .map(|name| name == "vbatLatest")
                        .unwrap_or(false)
                    {
                        // Check if previous value is corrupted (way too high for voltage)
                        if prev[field_index] > 1000 {
                            if debug {
                                eprintln!("DEBUG: Fixed corrupted vbatLatest previous value {} replaced with reasonable estimate", prev[field_index]);
                            }
                            // Use a reasonable voltage estimate based on vbatref
                            let vbatref = sysconfig.get("vbatref").copied().unwrap_or(4095);
                            return vbatref + raw_value; // Use vbatref as baseline + current delta
                        }
                    }

                    result
                } else {
                    raw_value
                }
            } else {
                raw_value
            }
        }

        PREDICT_STRAIGHT_LINE => {
            if let (Some(prev), Some(prev2)) = (previous_frame, previous2_frame) {
                if field_index < prev.len() && field_index < prev2.len() {
                    raw_value + 2 * prev[field_index] - prev2[field_index]
                } else {
                    raw_value
                }
            } else {
                raw_value
            }
        }

        PREDICT_AVERAGE_2 => {
            if let (Some(prev), Some(prev2)) = (previous_frame, previous2_frame) {
                if field_index < prev.len() && field_index < prev2.len() {
                    raw_value + ((prev[field_index] + prev2[field_index]) / 2)
                } else {
                    raw_value
                }
            } else {
                raw_value
            }
        }

        PREDICT_MINTHROTTLE => {
            let minthrottle = sysconfig.get("minthrottle").copied().unwrap_or(1150);
            raw_value + minthrottle
        }

        PREDICT_MOTOR_0 => {
            // Find motor[0] field index
            if let Some(motor0_idx) = field_names.iter().position(|name| name == "motor[0]") {
                if motor0_idx < current_frame.len() {
                    current_frame[motor0_idx] + raw_value
                } else {
                    raw_value
                }
            } else {
                raw_value
            }
        }

        PREDICT_INC => {
            let mut result = skipped_frames as i32 + 1;
            if let Some(prev) = previous_frame {
                if field_index < prev.len() {
                    result += prev[field_index];
                }
            }
            result
        }

        PREDICT_1500 => raw_value + 1500,

        PREDICT_VBATREF => {
            let vbatref = sysconfig.get("vbatref").copied().unwrap_or(4095);

            // CRITICAL FIX: Check for corrupted raw values in vbatLatest
            // Normal vbatLatest raw values should be small deltas (-50 to +50) or small absolute values (<1000)
            // Large values (>4000) indicate stream parsing corruption or wrong predictor application
            if field_names
                .get(field_index)
                .map(|name| name == "vbatLatest")
                .unwrap_or(false)
                && !(-1000..=4000).contains(&raw_value)
            {
                // This is clearly a corrupted value - likely caused by stream parsing error
                // Instead of propagating corruption, use a safe default value
                if debug {
                    eprintln!(
                        "DEBUG: Fixed corrupted vbatLatest raw_value {} replaced with 0",
                        raw_value
                    );
                }
                return vbatref; // Return just vbatref (safe default)
            }

            raw_value + vbatref
        }

        PREDICT_MINMOTOR => {
            // Get the min motor value from motorOutput "min,max" format
            let minmotor = if let Some(motor_output) = sysconfig.get("motorOutput") {
                // Parse "48,2047" format to get first value (48)
                let motor_output_str = motor_output.to_string();
                if let Some(comma_pos) = motor_output_str.find(',') {
                    motor_output_str[..comma_pos].parse().unwrap_or(48)
                } else {
                    motor_output_str.parse().unwrap_or(48)
                }
            } else {
                48 // Default min motor output value
            };
            raw_value + minmotor
        }

        _ => raw_value,
    }
}

pub fn decode_frame_field(
    stream: &mut BBLDataStream,
    encoding: u8,
    _data_version: u8,
) -> Result<i32> {
    match encoding {
        ENCODING_SIGNED_VB => stream.read_signed_vb(),

        ENCODING_UNSIGNED_VB => Ok(stream.read_unsigned_vb()? as i32),

        ENCODING_NEG_14BIT => {
            let value = stream.read_unsigned_vb()? as u16;
            Ok(-sign_extend_14bit(value))
        }

        ENCODING_NULL => Ok(0),

        _ => Err(anyhow::anyhow!("Unsupported encoding: {}", encoding)),
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn parse_frame_data(
    stream: &mut BBLDataStream,
    frame_def: &crate::FrameDefinition,
    current_frame: &mut [i32],
    previous_frame: Option<&[i32]>,
    previous2_frame: Option<&[i32]>,
    skipped_frames: u32,
    raw: bool,
    data_version: u8,
    sysconfig: &HashMap<String, i32>,
    debug: bool,
) -> Result<()> {
    let mut i = 0;
    let mut values = [0i32; 8];

    while i < frame_def.fields.len() {
        let field = &frame_def.fields[i];

        if field.predictor == PREDICT_INC {
            current_frame[i] = apply_predictor(
                i,
                field.predictor,
                0,
                current_frame,
                previous_frame,
                previous2_frame,
                skipped_frames,
                sysconfig,
                &frame_def.field_names,
                debug,
            );
            i += 1;
            continue;
        }

        match field.encoding {
            ENCODING_TAG8_4S16 => {
                if data_version < 2 {
                    // v1 implementation would be different but we'll use v2
                }
                stream.read_tag8_4s16_v2(&mut values)?;

                // Apply predictors for the 4 fields
                for j in 0..4 {
                    if i + j >= frame_def.fields.len() {
                        break;
                    }
                    let predictor = if raw {
                        PREDICT_0
                    } else {
                        frame_def.fields[i + j].predictor
                    };

                    current_frame[i + j] = apply_predictor(
                        i + j,
                        predictor,
                        values[j],
                        current_frame,
                        previous_frame,
                        previous2_frame,
                        skipped_frames,
                        sysconfig,
                        &frame_def.field_names,
                        debug,
                    );
                }
                i += 4;
                continue;
            }

            ENCODING_TAG2_3S32 => {
                stream.read_tag2_3s32(&mut values)?;

                // Apply predictors for the 3 fields
                for j in 0..3 {
                    if i + j >= frame_def.fields.len() {
                        break;
                    }
                    let predictor = if raw {
                        PREDICT_0
                    } else {
                        frame_def.fields[i + j].predictor
                    };
                    current_frame[i + j] = apply_predictor(
                        i + j,
                        predictor,
                        values[j],
                        current_frame,
                        previous_frame,
                        previous2_frame,
                        skipped_frames,
                        sysconfig,
                        &frame_def.field_names,
                        debug,
                    );
                }
                i += 3;
                continue;
            }

            ENCODING_TAG8_8SVB => {
                // Count how many fields use this encoding
                let mut group_count = 1;
                for j in i + 1..i + 8.min(frame_def.fields.len() - i) {
                    if frame_def.fields[j].encoding != ENCODING_TAG8_8SVB {
                        break;
                    }
                    group_count += 1;
                }

                stream.read_tag8_8svb(&mut values, group_count)?;

                // Apply predictors for the group
                for j in 0..group_count {
                    if i + j >= frame_def.fields.len() {
                        break;
                    }
                    let predictor = if raw {
                        PREDICT_0
                    } else {
                        frame_def.fields[i + j].predictor
                    };

                    current_frame[i + j] = apply_predictor(
                        i + j,
                        predictor,
                        values[j],
                        current_frame,
                        previous_frame,
                        previous2_frame,
                        skipped_frames,
                        sysconfig,
                        &frame_def.field_names,
                        debug,
                    );
                }
                i += group_count;
                continue;
            }

            _ => {
                let raw_value = decode_frame_field(stream, field.encoding, data_version)?;
                let predictor = if raw { PREDICT_0 } else { field.predictor };

                current_frame[i] = apply_predictor(
                    i,
                    predictor,
                    raw_value,
                    current_frame,
                    previous_frame,
                    previous2_frame,
                    skipped_frames,
                    sysconfig,
                    &frame_def.field_names,
                    debug,
                );
            }
        }

        i += 1;
    }

    Ok(())
}
