//! Event frame parsing helper module
//!
//! Contains functions for parsing E-frames (Event data) from blackbox log data.
//! These helpers are used by both the library parser and CLI binary.

use crate::parser::stream::BBLDataStream;
use crate::types::EventFrame;
use crate::Result;

/// Helper function to parse inflight adjustment events (types 4 and 13)
/// Returns the event description string
fn parse_inflight_adjustment(
    stream: &mut BBLDataStream,
    event_data: &mut Vec<u8>,
) -> Result<String> {
    let adjustment_function = stream.read_byte()?;
    event_data.extend_from_slice(&[adjustment_function]);
    if adjustment_function > 127 {
        let new_value = stream.read_unsigned_vb()? as f32;
        Ok(format!(
            "Inflight adjustment - Function: {}, New value: {:.3}",
            adjustment_function, new_value
        ))
    } else {
        let new_value = stream.read_signed_vb()?;
        Ok(format!(
            "Inflight adjustment - Function: {}, New value: {}",
            adjustment_function, new_value
        ))
    }
}

/// Parse E-frame (Event frame) data from the stream
///
/// E-frames contain various event types such as sync beeps, autotune cycles,
/// inflight adjustments, logging resume, disarm, flight mode changes, and log end.
/// Each event type has its own data format that this function decodes.
pub fn parse_e_frame(stream: &mut BBLDataStream, debug: bool) -> Result<EventFrame> {
    if debug {
        println!("Parsing E frame (Event frame)");
    }

    // Read event type (1 byte)
    let event_type = stream.read_byte()?;

    // Read event data - the length depends on the event type
    let mut event_data = Vec::new();
    let event_name = match event_type {
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
            let axis = stream.read_byte()?;
            let p_gain = stream.read_signed_vb()? as f32 / 1000.0;
            let i_gain = stream.read_signed_vb()? as f32 / 1000.0;
            let d_gain = stream.read_signed_vb()? as f32 / 1000.0;
            event_data.extend_from_slice(&[axis]);
            format!(
                "Autotune cycle result - Axis: {}, P: {:.3}, I: {:.3}, D: {:.3}",
                axis, p_gain, i_gain, d_gain
            )
        }
        3 => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_TARGETS
            let current_angle = stream.read_signed_vb()?;
            let target_angle = stream.read_signed_vb()?;
            let target_angle_at_peak = stream.read_signed_vb()?;
            let first_peak_angle = stream.read_signed_vb()?;
            let second_peak_angle = stream.read_signed_vb()?;
            format!(
                "Autotune targets - Current: {}, Target: {}, Peak target: {}, First peak: {}, Second peak: {}",
                current_angle, target_angle, target_angle_at_peak, first_peak_angle, second_peak_angle
            )
        }
        4 => {
            // FLIGHT_LOG_EVENT_INFLIGHT_ADJUSTMENT
            parse_inflight_adjustment(stream, &mut event_data)?
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
                    event_data.push(stream.read_byte()?);
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
            parse_inflight_adjustment(stream, &mut event_data)?
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
                    event_data.push(stream.read_byte()?);
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
                event_data.push(stream.read_byte()?);
            }
            format!("Unknown event type: {}", event_type)
        }
    };

    if debug {
        println!(
            "DEBUG: Event - Type: {}, Description: {}",
            event_type, event_name
        );
    }

    Ok(EventFrame {
        timestamp_us: 0, // Will be set later from context
        event_type,
        event_data,
        event_name,
    })
}
