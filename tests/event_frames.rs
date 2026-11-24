//! Event frame decoding through the public parser API.
//!
//! Fixtures mirror blackbox-tools `test/test_events.sh` (PR 78): two I frames
//! with one event frame between them, then a clean-end event. Expected
//! payload layouts come from Betaflight `blackbox.c` `blackboxLogEvent`.

use bbl_parser::{parse_single_log, ExportOptions};

const HEADER: &[u8] = b"H Product:Blackbox flight data recorder by Nicholas Sherlock\n\
H Data version:2\n\
H Field I name:loopIteration,time\n\
H Field I signed:0,0\n\
H Field I predictor:0,0\n\
H Field I encoding:1,1\n";

fn iframe(iteration: u8, time_vb: &[u8]) -> Vec<u8> {
    let mut v = vec![b'I', iteration];
    v.extend_from_slice(time_vb);
    v
}

fn log_with_event(event: &[u8]) -> Vec<u8> {
    let mut data = HEADER.to_vec();
    data.extend(iframe(0, &[0xD0, 0x0F])); // time 2000
    data.extend_from_slice(event);
    data.extend(iframe(1, &[0xB8, 0x17])); // time 3000
    data
}

fn parse(data: &[u8]) -> bbl_parser::BBLLog {
    let opts = ExportOptions {
        event: true,
        ..Default::default()
    };
    parse_single_log(data, 1, 1, false, &opts).unwrap()
}

fn i_frames(log: &bbl_parser::BBLLog) -> usize {
    log.frames.iter().filter(|f| f.frame_type == 'I').count()
}

fn names(log: &bbl_parser::BBLLog) -> Vec<String> {
    log.event_frames
        .iter()
        .map(|e| e.event_name.clone())
        .collect()
}

#[test]
fn disarm_event_is_decoded_and_stream_stays_in_sync() {
    let log = parse(&log_with_event(&[b'E', 15, 0x02]));
    assert_eq!(i_frames(&log), 2);
    assert_eq!(names(&log)[0], "Disarm - Reason: 2");
    assert_eq!(log.event_frames[0].timestamp_us, 2000);
}

#[test]
fn flightmode_event_with_four_byte_flags() {
    let log = parse(&log_with_event(&[b'E', 30, 0x81, 0x80, 0x80, 0x08, 0x01]));
    assert_eq!(i_frames(&log), 2);
    assert_eq!(
        names(&log)[0],
        "Flight mode - Flags: 16777217, Last flags: 1"
    );
}

#[test]
fn flightmode_flags_73_does_not_swallow_an_i_frame() {
    // flags = 73 encodes to 0x49 ('I'), the I-frame marker byte.
    let log = parse(&log_with_event(&[b'E', 30, 73, 0x01]));
    assert_eq!(i_frames(&log), 2, "both I frames must survive");
    assert_eq!(names(&log)[0], "Flight mode - Flags: 73, Last flags: 1");
}

#[test]
fn flightmode_flags_with_bit_31() {
    let log = parse(&log_with_event(&[
        b'E', 30, 0x80, 0x80, 0x80, 0x80, 0x08, 0x01,
    ]));
    assert_eq!(i_frames(&log), 2);
    assert_eq!(
        names(&log)[0],
        "Flight mode - Flags: 2147483648, Last flags: 1"
    );
}

#[test]
fn truncated_disarm_payload_at_end_of_data_does_not_panic() {
    let mut data = HEADER.to_vec();
    data.extend(iframe(0, &[0xD0, 0x0F]));
    data.extend_from_slice(&[b'E', 15]); // payload missing
    let log = parse(&data);
    assert_eq!(i_frames(&log), 1);
}
