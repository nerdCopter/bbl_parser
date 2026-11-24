//! Event frame parsing helper module
//!
//! Contains functions for parsing E-frames (Event data) from blackbox log data.
//! These helpers are used by both the library parser and CLI binary.

use crate::firmware_tables::*;
use crate::parser::stream::BBLDataStream;
use crate::types::EventFrame;
use anyhow::Result;

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
        EVENT_SYNC_BEEP => {
            // FLIGHT_LOG_EVENT_SYNC_BEEP
            "Sync beep".to_string()
        }
        EVENT_AUTOTUNE_CYCLE_START => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_START
            "Autotune cycle start".to_string()
        }
        EVENT_AUTOTUNE_CYCLE_RESULT => {
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
        EVENT_AUTOTUNE_TARGETS => {
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
        EVENT_INFLIGHT_ADJUSTMENT_OLD => {
            // FLIGHT_LOG_EVENT_INFLIGHT_ADJUSTMENT
            parse_inflight_adjustment(stream, &mut event_data)?
        }
        EVENT_LOGGING_RESUME_OLD => {
            // FLIGHT_LOG_EVENT_LOGGING_RESUME
            // Note: Event type 14 has identical implementation - both use this newer numbering
            let log_iteration = stream.read_unsigned_vb()?;
            let current_time = stream.read_unsigned_vb()?;
            format!(
                "Logging resume - Iteration: {}, Time: {}",
                log_iteration, current_time
            )
        }
        EVENT_LOG_END_OLD => {
            // FLIGHT_LOG_EVENT_LOG_END (old numbering)
            // Read end message bytes
            for _ in 0..4 {
                if !stream.eof {
                    event_data.push(stream.read_byte()?);
                }
            }
            "Log end".to_string()
        }
        EVENT_AUTOTUNE_CYCLE_START_UNUSED => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_START (UNUSED)
            "Autotune cycle start (unused)".to_string()
        }
        EVENT_AUTOTUNE_CYCLE_RESULT_UNUSED => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_CYCLE_RESULT (UNUSED)
            "Autotune cycle result (unused)".to_string()
        }
        EVENT_AUTOTUNE_TARGETS_UNUSED => {
            // FLIGHT_LOG_EVENT_AUTOTUNE_TARGETS (UNUSED)
            "Autotune targets (unused)".to_string()
        }
        EVENT_INFLIGHT_ADJUSTMENT => {
            // FLIGHT_LOG_EVENT_INFLIGHT_ADJUSTMENT
            parse_inflight_adjustment(stream, &mut event_data)?
        }
        EVENT_LOGGING_RESUME => {
            // FLIGHT_LOG_EVENT_LOGGING_RESUME (newer numbering, same as type 5)
            // Both event type numbers are used in different firmware versions
            let log_iteration = stream.read_unsigned_vb()?;
            let current_time = stream.read_unsigned_vb()?;
            format!(
                "Logging resume - Iteration: {}, Time: {}",
                log_iteration, current_time
            )
        }
        EVENT_DISARM => {
            // FLIGHT_LOG_EVENT_DISARM: one unsigned VB, the disarm reason
            let reason = stream.read_unsigned_vb()?;
            format!("{} - Reason: {}", EVENT_NAME_DISARM, reason)
        }
        EVENT_FLIGHTMODE => {
            // FLIGHT_LOG_EVENT_FLIGHTMODE: two unsigned VBs, flags then lastFlags
            let flags = stream.read_unsigned_vb()?;
            let last_flags = stream.read_unsigned_vb()?;
            format!(
                "{} - Flags: {}, Last flags: {}",
                EVENT_NAME_FLIGHT_MODE, flags, last_flags
            )
        }
        EVENT_LOG_END => {
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

#[cfg(test)]
mod tests {
    use super::*;

    // Payloads follow Betaflight blackbox.c blackboxLogEvent (60abbac05):
    // DISARM writes one unsigned VB; FLIGHTMODE writes two (flags, lastFlags).

    #[test]
    fn disarm_reads_one_unsigned_vb() {
        let data = [EVENT_DISARM, 0x02, 0xAA];
        let mut stream = BBLDataStream::new(&data);
        let frame = parse_e_frame(&mut stream, false).unwrap();
        assert_eq!(frame.event_name, "Disarm - Reason: 2");
        assert_eq!(
            stream.pos, 2,
            "payload byte must be consumed, 0xAA must not"
        );
    }

    #[test]
    fn flightmode_reads_two_unsigned_vbs() {
        // flags = 16777217 -> 81 80 80 08, lastFlags = 1 -> 01
        let data = [EVENT_FLIGHTMODE, 0x81, 0x80, 0x80, 0x08, 0x01, 0xAA];
        let mut stream = BBLDataStream::new(&data);
        let frame = parse_e_frame(&mut stream, false).unwrap();
        assert_eq!(
            frame.event_name,
            "Flight mode - Flags: 16777217, Last flags: 1"
        );
        assert_eq!(stream.pos, 6);
    }

    #[test]
    fn flightmode_flags_with_bit_31_use_five_byte_vb() {
        // 0x80000000 -> 80 80 80 80 08
        let data = [EVENT_FLIGHTMODE, 0x80, 0x80, 0x80, 0x80, 0x08, 0x01];
        let mut stream = BBLDataStream::new(&data);
        let frame = parse_e_frame(&mut stream, false).unwrap();
        assert_eq!(
            frame.event_name,
            "Flight mode - Flags: 2147483648, Last flags: 1"
        );
        assert_eq!(stream.pos, 7);
    }

    #[test]
    fn flightmode_truncated_payload_does_not_panic() {
        // Payload cut after the first VB. The stream reader returns 0 at EOF,
        // as streamReadUnsignedVB does upstream.
        let data = [EVENT_FLIGHTMODE, 0x05];
        let mut stream = BBLDataStream::new(&data);
        let frame = parse_e_frame(&mut stream, false).unwrap();
        assert_eq!(frame.event_name, "Flight mode - Flags: 5, Last flags: 0");
        assert!(stream.eof);
    }

    #[test]
    fn disarm_with_no_payload_does_not_panic() {
        let data = [EVENT_DISARM];
        let mut stream = BBLDataStream::new(&data);
        let frame = parse_e_frame(&mut stream, false).unwrap();
        assert_eq!(frame.event_name, "Disarm - Reason: 0");
    }
}
