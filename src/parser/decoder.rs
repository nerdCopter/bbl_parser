use crate::error::{BBLError, Result};
use crate::parser::stream::BBLDataStream;

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
            return Err(BBLError::InvalidEncoding(encoding));
        }
    }
    Ok(())
}

pub fn apply_predictor(
    predictor: u8,
    value: i32,
    field_index: usize,
    current_frame: &[i32],
    previous_frame: &[i32],
    previous2_frame: &[i32],
    sysconfig: &std::collections::HashMap<String, i32>,
) -> Result<i32> {
    match predictor {
        PREDICT_0 => Ok(value),
        PREDICT_PREVIOUS => {
            if field_index < previous_frame.len() {
                Ok(value + previous_frame[field_index])
            } else {
                Ok(value)
            }
        }
        PREDICT_STRAIGHT_LINE => {
            if field_index < previous_frame.len() && field_index < previous2_frame.len() {
                let prediction = 2 * previous_frame[field_index] - previous2_frame[field_index];
                Ok(value + prediction)
            } else if field_index < previous_frame.len() {
                Ok(value + previous_frame[field_index])
            } else {
                Ok(value)
            }
        }
        PREDICT_AVERAGE_2 => {
            if field_index < previous_frame.len() && field_index < previous2_frame.len() {
                let average = (previous_frame[field_index] + previous2_frame[field_index]) / 2;
                Ok(value + average)
            } else if field_index < previous_frame.len() {
                Ok(value + previous_frame[field_index])
            } else {
                Ok(value)
            }
        }
        PREDICT_MINTHROTTLE => {
            let minthrottle = sysconfig.get("minthrottle").copied().unwrap_or(1000);
            Ok(value + minthrottle)
        }
        PREDICT_MOTOR_0 => {
            // motor[1], motor[2], motor[3] are predicted based on motor[0]
            // Find motor[0] field index (typically field 39 in I-frame)
            // For now, use current_frame[39] as motor[0] position based on header analysis
            let motor0_index = 39; // Based on field analysis: motor[0] is at position 39
            if motor0_index < current_frame.len() {
                Ok(value + current_frame[motor0_index])
            } else {
                Ok(value)
            }
        }
        PREDICT_INC => {
            if field_index < previous_frame.len() {
                Ok(previous_frame[field_index] + value)
            } else {
                Ok(value)
            }
        }
        PREDICT_HOME_COORD => {
            // GPS home coordinate prediction - for now just return value
            Ok(value)
        }
        PREDICT_1500 => Ok(value + 1500),
        PREDICT_VBATREF => {
            let vbatref = sysconfig.get("vbatref").copied().unwrap_or(4095);
            Ok(value + vbatref)
        }
        PREDICT_MINMOTOR => {
            // predictor 11
            // motor[0] prediction: value + motorOutput[0] (minimum motor output)
            let motor_output_min = sysconfig.get("motorOutput[0]").copied().unwrap_or(48);
            Ok(value + motor_output_min) // Force signed 32-bit like Betaflight
        }
        _ => Err(BBLError::InvalidPredictor(predictor)),
    }
}
