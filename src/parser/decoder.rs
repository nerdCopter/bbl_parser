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

/// Parse H-frame (GPS Home) data
pub fn parse_h_frame(
    stream: &mut BBLDataStream,
    frame_def: &crate::types::FrameDefinition,
    debug: bool,
) -> Result<std::collections::HashMap<String, i32>> {
    let mut data = std::collections::HashMap::new();

    if debug {
        println!("Parsing H frame with {} fields", frame_def.count);
    }

    // H frames contain GPS home position data
    for (i, field) in frame_def.fields.iter().enumerate() {
        if i >= frame_def.count {
            break;
        }

        let value = match field.encoding {
            ENCODING_SIGNED_VB => stream.read_signed_vb()?,
            ENCODING_UNSIGNED_VB => stream.read_unsigned_vb()? as i32,
            ENCODING_NEG_14BIT => stream.read_neg_14bit()?,
            ENCODING_NULL => 0,
            _ => {
                if debug {
                    println!(
                        "Unsupported H-frame encoding {} for field {}",
                        field.encoding, field.name
                    );
                }
                stream.read_signed_vb().unwrap_or(0)
            }
        };

        data.insert(field.name.clone(), value);
    }

    Ok(data)
}

/// Parse E-frame (Event) data based on C reference implementation
pub fn parse_e_frame(stream: &mut BBLDataStream, debug: bool) -> Result<crate::types::EventFrame> {
    if debug {
        println!("Parsing E frame (Event frame)");
    }

    // Read event type (1 byte)
    let event_type = stream.read_byte()?;

    // Read event data - the length depends on the event_type
    let event_data = Vec::new();
    let event_description = match event_type {
        0 => {
            // FLIGHT_LOG_EVENT_SYNC_BEEP
            "Sync beep".to_string()
        }
        1 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_START
            "Autotune cycle start".to_string()
        }
        2 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_RESULT
            let _axis = stream.read_byte()?;
            let p_gain = stream.read_signed_vb()? as f32 / 1000.0;
            let i_gain = stream.read_signed_vb()? as f32 / 1000.0;
            let d_gain = stream.read_signed_vb()? as f32 / 1000.0;
            format!(
                "Autotune cycle result - Axis: {}, P: {:.3}, I: {:.3}, D: {:.3}",
                _axis, p_gain, i_gain, d_gain
            )
        }
        3 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_TARGETS
            let current_angle = stream.read_signed_vb()?;
            let target_angle = stream.read_signed_vb()?;
            let target_angle_at_peak = stream.read_signed_vb()?;
            let first_peak_angle = stream.read_signed_vb()?;
            let second_peak_angle = stream.read_signed_vb()?;
            format!("Autotune targets - Current: {}, Target: {}, Peak target: {}, First peak: {}, Second peak: {}", 
                   current_angle, target_angle, target_angle_at_peak, first_peak_angle, second_peak_angle)
        }
        4 => {
            // FLIGHT_LOG_EVENT_INFLIGHT_ADJUSTMENT
            let adjustment_function = stream.read_byte()?;
            if adjustment_function > 127 {
                // Float value
                let new_value = stream.read_unsigned_vb()? as f32;
                format!(
                    "Inflight adjustment - Function: {}, New value: {:.3}",
                    adjustment_function, new_value
                )
            } else {
                // Integer value
                let new_value = stream.read_signed_vb()?;
                format!(
                    "Inflight adjustment - Function: {}, New value: {}",
                    adjustment_function, new_value
                )
            }
        }
        5 => {
            // FLIGHT_LOG_EVENT_LOGGING_RESUME
            let log_iteration = stream.read_unsigned_vb()?;
            let current_time = stream.read_unsigned_vb()?;
            format!(
                "Logging resume - Iteration: {}, Time: {}",
                log_iteration, current_time
            )
        }
        6 => {
            // FLIGHT_LOG_EVENT_LOG_END (old numbering)
            // Read end message bytes
            for _ in 0..4 {
                if !stream.eof {
                    let _ = stream.read_byte()?;
                }
            }
            "Log end".to_string()
        }
        10 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_START (UNUSED)
            "Autotune cycle start (unused)".to_string()
        }
        11 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_RESULT (UNUSED)
            "Autotune cycle result (unused)".to_string()
        }
        12 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_TARGETS (UNUSED)
            "Autotune targets (unused)".to_string()
        }
        13 => {
            // FLIGHT_LOG_EVENT_INFLIGHT_ADJUSTMENT
            let adjustment_function = stream.read_byte()?;
            if adjustment_function > 127 {
                let new_value = stream.read_unsigned_vb()? as f32;
                format!(
                    "Inflight adjustment - Function: {}, New value: {:.3}",
                    adjustment_function, new_value
                )
            } else {
                let new_value = stream.read_signed_vb()?;
                format!(
                    "Inflight adjustment - Function: {}, New value: {}",
                    adjustment_function, new_value
                )
            }
        }
        14 => {
            // FLIGHT_LOG_EVENT_LOGGING_RESUME
            let log_iteration = stream.read_unsigned_vb()?;
            let current_time = stream.read_unsigned_vb()?;
            format!(
                "Logging resume - Iteration: {}, Time: {}",
                log_iteration, current_time
            )
        }
        15 => {
            // FLIGHT_LOG_EVENT_DISARM
            "Disarm".to_string()
        }
        30 => {
            // FLIGHT_LOG_EVENT_FLIGHTMODE - flight mode status event
            // Read flight mode data
            for _ in 0..4 {
                if !stream.eof {
                    let _ = stream.read_byte()?;
                }
            }
            "Flight mode change".to_string()
        }
        255 => {
            // FLIGHT_LOG_EVENT_LOG_END
            "Log end".to_string()
        }
        _ => {
            // Unknown event type - read a few bytes as data
            for _ in 0..8 {
                if stream.eof {
                    break;
                }
                let _ = stream.read_byte()?;
            }
            format!("Unknown event type: {}", event_type)
        }
    };

    if debug {
        println!(
            "DEBUG: Event - Type: {}, Description: {}",
            event_type, event_description
        );
    }

    Ok(crate::types::EventFrame {
        timestamp_us: 0, // Will be set later from context
        event_type,
        event_data,
        event_description,
    })
}
