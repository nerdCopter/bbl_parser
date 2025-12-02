use crate::parser::stream::BBLDataStream;
use anyhow::Result;

// BBL Encoding constants - directly from JavaScript reference
pub const ENCODING_SIGNED_VB: u8 = 0;
pub const ENCODING_UNSIGNED_VB: u8 = 1;
pub const ENCODING_NEG_14BIT: u8 = 3;
pub const ENCODING_TAG8_8SVB: u8 = 6;
pub const ENCODING_TAG2_3S32: u8 = 7;
pub const ENCODING_TAG8_4S16: u8 = 8;
pub const ENCODING_NULL: u8 = 9;
pub const ENCODING_TAG2_3SVARIABLE: u8 = 10;

// Predictor constants - directly from JavaScript reference
pub const PREDICT_0: u8 = 0;
pub const PREDICT_PREVIOUS: u8 = 1;
pub const PREDICT_STRAIGHT_LINE: u8 = 2;
pub const PREDICT_AVERAGE_2: u8 = 3;
pub const PREDICT_MINTHROTTLE: u8 = 4;
pub const PREDICT_MOTOR_0: u8 = 5;
pub const PREDICT_INC: u8 = 6;
pub const PREDICT_HOME_COORD: u8 = 7;
pub const PREDICT_1500: u8 = 8;
pub const PREDICT_VBATREF: u8 = 9;
pub const PREDICT_LAST_MAIN_FRAME_TIME: u8 = 10;
pub const PREDICT_MINMOTOR: u8 = 11;

// Domain-specific constants for corruption detection
// Maximum reasonable raw vbatLatest value before considering it corrupted
const MAX_REASONABLE_VBAT_RAW: i32 = 1000;

/// Decode a field value using the specified encoding
pub fn decode_field_value(
    stream: &mut BBLDataStream,
    encoding: u8,
    values: &mut [i32],
    index: usize,
) -> Result<()> {
    match encoding {
        ENCODING_SIGNED_VB => {
            values[index] = stream.read_signed_vb()?;
        }
        ENCODING_UNSIGNED_VB => {
            values[index] = stream.read_unsigned_vb()? as i32;
        }
        ENCODING_NEG_14BIT => {
            values[index] = stream.read_neg_14bit()?;
        }
        ENCODING_NULL => {
            values[index] = 0;
        }
        _ => {
            return Err(anyhow::anyhow!("Invalid encoding type: {}", encoding));
        }
    }
    Ok(())
}

/// Apply predictor to decode frame field value
/// Enhanced version with debug support, field names lookup, and corruption prevention
#[allow(clippy::too_many_arguments)]
pub fn apply_predictor(
    predictor: u8,
    value: i32,
    field_index: usize,
    current_frame: &[i32],
    previous_frame: &[i32],
    previous2_frame: &[i32],
    sysconfig: &std::collections::HashMap<String, i32>,
) -> Result<i32> {
    // Call the enhanced version with default parameters
    Ok(apply_predictor_with_debug(
        field_index,
        predictor,
        value,
        current_frame,
        Some(previous_frame),
        Some(previous2_frame),
        0,
        sysconfig,
        &[],
        false,
    ))
}

/// Enhanced apply_predictor with debug support, field names lookup, and corruption prevention
/// This matches the CLI implementation's full feature set
#[allow(clippy::too_many_arguments)]
pub fn apply_predictor_with_debug(
    field_index: usize,
    predictor: u8,
    raw_value: i32,
    current_frame: &[i32],
    previous_frame: Option<&[i32]>,
    previous2_frame: Option<&[i32]>,
    skipped_frames: u32,
    sysconfig: &std::collections::HashMap<String, i32>,
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
                        if prev[field_index] > MAX_REASONABLE_VBAT_RAW {
                            if debug {
                                eprintln!("DEBUG: Fixed corrupted vbatLatest previous value {} replaced with reasonable estimate", prev[field_index]);
                            }
                            // Use a reasonable voltage estimate based on vbatref
                            let vbatref = sysconfig.get("vbatref").copied().unwrap_or(4095);
                            return vbatref + raw_value;
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
            // Find motor[0] field index dynamically if field_names available
            if !field_names.is_empty() {
                if let Some(motor0_idx) = field_names.iter().position(|name| name == "motor[0]") {
                    if motor0_idx < current_frame.len() {
                        return current_frame[motor0_idx] + raw_value;
                    }
                }
            }
            // Fallback: use hardcoded position (typically field 39 in I-frame)
            let motor0_index = 39;
            if motor0_index < current_frame.len() {
                if debug {
                    eprintln!(
                        "DEBUG: PREDICT_MOTOR_0 using hardcoded fallback index {}",
                        motor0_index
                    );
                }
                current_frame[motor0_index] + raw_value
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

        PREDICT_HOME_COORD => {
            // GPS home coordinate prediction - for now just return value
            raw_value
        }

        PREDICT_1500 => raw_value + 1500,

        PREDICT_VBATREF => {
            let vbatref = sysconfig.get("vbatref").copied().unwrap_or(4095);

            // CRITICAL FIX: Check for corrupted raw values in vbatLatest
            if !field_names.is_empty()
                && field_names
                    .get(field_index)
                    .map(|name| name == "vbatLatest")
                    .unwrap_or(false)
                && !(-1000..=4000).contains(&raw_value)
            {
                if debug {
                    eprintln!(
                        "DEBUG: Fixed corrupted vbatLatest raw_value {} replaced with 0",
                        raw_value
                    );
                }
                return vbatref;
            }

            raw_value + vbatref
        }

        PREDICT_MINMOTOR => {
            // Get the min motor value from motorOutput[0] or motorOutput
            let minmotor = sysconfig
                .get("motorOutput[0]")
                .or_else(|| sysconfig.get("motorOutput"))
                .copied()
                .unwrap_or(48);
            raw_value + minmotor
        }

        _ => raw_value,
    }
}
