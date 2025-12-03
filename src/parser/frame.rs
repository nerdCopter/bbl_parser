use crate::conversion::{
    convert_gps_altitude, convert_gps_coordinate, convert_gps_course, convert_gps_speed,
};
use crate::parser::{
    decoder::apply_predictor_with_debug, decoder::*, event::parse_e_frame, gps::*,
    stream::BBLDataStream,
};
use crate::types::{
    DecodedFrame, EventFrame, FrameDefinition, FrameHistory, FrameStats, GpsCoordinate,
    GpsHomeCoordinate,
};
use crate::ExportOptions;
use anyhow::Result;
use std::collections::HashMap;
use std::io::Write;

/// Parse frames from binary data
///
/// Parses ALL frames from binary data and stores them for CSV export.
/// This is the unified implementation used by both CLI and crate.
///
/// # Arguments
/// * `binary_data` - Raw binary frame data
/// * `header` - Parsed BBL header with frame definitions
/// * `debug` - Enable debug output
/// * `export_options` - Export options controlling GPS/event collection
#[allow(clippy::type_complexity)]
pub fn parse_frames(
    binary_data: &[u8],
    header: &crate::types::BBLHeader,
    debug: bool,
    export_options: &ExportOptions,
) -> Result<(
    FrameStats,
    Vec<DecodedFrame>,
    Option<HashMap<char, Vec<DecodedFrame>>>,
    Vec<GpsCoordinate>,
    Vec<GpsHomeCoordinate>,
    Vec<EventFrame>,
)> {
    let mut stats = FrameStats::default();
    let mut frames = Vec::new();
    let mut debug_frames: HashMap<char, Vec<DecodedFrame>> = HashMap::new();
    let mut last_main_frame_timestamp = 0u64; // Track timestamp for S frames

    // Track the most recent S-frame data for merging (following JavaScript approach)
    let mut last_slow_data: HashMap<String, i32> = HashMap::new();

    if debug {
        println!("Binary data size: {} bytes", binary_data.len());
        if !binary_data.is_empty() {
            println!(
                "First 16 bytes: {:02X?}",
                &binary_data[..16.min(binary_data.len())]
            );
        }
    }

    if binary_data.is_empty() {
        return Ok((
            stats,
            frames,
            Some(debug_frames),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ));
    }

    // Initialize frame history for proper P-frame parsing
    let mut frame_history = FrameHistory {
        current_frame: vec![0; header.i_frame_def.count],
        previous_frame: vec![0; header.i_frame_def.count],
        previous2_frame: vec![0; header.i_frame_def.count],
        valid: false,
    };

    // Collections for GPS and Event export
    let mut gps_coordinates: Vec<GpsCoordinate> = Vec::new();
    let mut home_coordinates: Vec<GpsHomeCoordinate> = Vec::new();
    let mut event_frames: Vec<EventFrame> = Vec::new();

    // GPS frame history for differential encoding
    let mut gps_frame_history: Vec<i32> = Vec::new();

    let mut stream = BBLDataStream::new(binary_data);

    // Main frame parsing loop - process frames as a stream
    while !stream.eof {
        let frame_start_pos = stream.pos;

        match stream.read_byte() {
            Ok(frame_type_byte) => {
                let frame_type = match frame_type_byte as char {
                    'I' => 'I',
                    'P' => 'P',
                    'H' => 'H',
                    'G' => 'G',
                    'E' => 'E',
                    'S' => 'S',
                    _ => {
                        if debug && stats.failed_frames < 3 {
                            println!(
                                "Unknown frame type byte 0x{:02X} ('{:?}') at offset {}",
                                frame_type_byte, frame_type_byte as char, frame_start_pos
                            );
                        }
                        stats.failed_frames += 1;
                        continue;
                    }
                };

                if debug && stats.total_frames < 3 {
                    println!("Found frame type '{frame_type}' at offset {frame_start_pos}");
                }

                // Parse frame using proper streaming logic
                let mut frame_data = HashMap::new();
                let mut parsing_success = false;

                match frame_type {
                    'I' => {
                        if header.i_frame_def.count > 0 {
                            // I-frames reset the prediction history
                            frame_history.current_frame.fill(0);

                            if parse_frame_data(
                                &mut stream,
                                &header.i_frame_def,
                                &mut frame_history.current_frame,
                                None, // I-frames don't use prediction
                                None,
                                0,
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                                debug,
                            )
                            .is_ok()
                            {
                                // Update time and loop iteration from parsed frame
                                for (i, field_name) in
                                    header.i_frame_def.field_names.iter().enumerate()
                                {
                                    if i < frame_history.current_frame.len() {
                                        let value = frame_history.current_frame[i];
                                        frame_data.insert(field_name.clone(), value);
                                    }
                                }

                                // Merge lastSlow data into I-frame (following JavaScript approach)
                                for (key, value) in &last_slow_data {
                                    frame_data.insert(key.clone(), *value);
                                }

                                if debug && stats.i_frames < 3 {
                                    println!("DEBUG: I-frame merged lastSlow. rxSignalReceived: {:?}, rxFlightChannelsValid: {:?}", 
                                             frame_data.get("rxSignalReceived"), frame_data.get("rxFlightChannelsValid"));
                                }

                                // Update history for future P-frames
                                frame_history
                                    .previous_frame
                                    .copy_from_slice(&frame_history.current_frame);
                                frame_history
                                    .previous2_frame
                                    .copy_from_slice(&frame_history.current_frame);
                                frame_history.valid = true;

                                // Validate frame before accepting
                                let current_time =
                                    frame_data.get("time").copied().unwrap_or(0) as u64;
                                let current_loop =
                                    frame_data.get("loopIteration").copied().unwrap_or(0) as u32;

                                let is_valid_frame =
                                    current_time > 0 && (current_loop > 0 || current_time > 1000);

                                if is_valid_frame {
                                    parsing_success = true;
                                    stats.i_frames += 1;

                                    if debug && stats.i_frames <= 3 {
                                        println!(
                                            "DEBUG: Accepted I-frame - time:{}, loop:{}",
                                            current_time, current_loop
                                        );
                                    }
                                } else if debug && stats.i_frames < 5 {
                                    println!(
                                        "DEBUG: Rejected I-frame - time:{}, loop:{} (invalid)",
                                        current_time, current_loop
                                    );
                                }
                            }
                        }
                    }
                    'P' => {
                        if header.p_frame_def.count > 0 && frame_history.valid {
                            let mut p_frame_values = vec![0i32; header.p_frame_def.count];

                            if parse_frame_data(
                                &mut stream,
                                &header.p_frame_def,
                                &mut p_frame_values,
                                Some(&frame_history.previous_frame),
                                Some(&frame_history.previous2_frame),
                                0,
                                false,
                                header.data_version,
                                &header.sysconfig,
                                debug,
                            )
                            .is_ok()
                            {
                                // Copy previous frame as base, then update P-frame fields
                                frame_history
                                    .current_frame
                                    .copy_from_slice(&frame_history.previous_frame);

                                // Update only the fields present in P-frame
                                for (i, field_name) in
                                    header.p_frame_def.field_names.iter().enumerate()
                                {
                                    if i < p_frame_values.len() {
                                        if let Some(i_frame_idx) = header
                                            .i_frame_def
                                            .field_names
                                            .iter()
                                            .position(|name| name == field_name)
                                        {
                                            if i_frame_idx < frame_history.current_frame.len() {
                                                frame_history.current_frame[i_frame_idx] =
                                                    p_frame_values[i];
                                            }
                                        }
                                    }
                                }

                                // Copy current frame to output
                                for (i, field_name) in
                                    header.i_frame_def.field_names.iter().enumerate()
                                {
                                    if i < frame_history.current_frame.len() {
                                        let value = frame_history.current_frame[i];
                                        frame_data.insert(field_name.clone(), value);
                                    }
                                }

                                // Merge lastSlow data
                                for (key, value) in &last_slow_data {
                                    frame_data.insert(key.clone(), *value);
                                }

                                if debug && stats.p_frames < 3 {
                                    println!("DEBUG: P-frame merged lastSlow. rxSignalReceived: {:?}, rxFlightChannelsValid: {:?}", 
                                             frame_data.get("rxSignalReceived"), frame_data.get("rxFlightChannelsValid"));
                                }

                                // Update history
                                frame_history
                                    .previous2_frame
                                    .copy_from_slice(&frame_history.previous_frame);
                                frame_history
                                    .previous_frame
                                    .copy_from_slice(&frame_history.current_frame);

                                // Validate P-frame
                                let current_time =
                                    frame_data.get("time").copied().unwrap_or(0) as u64;
                                let current_loop =
                                    frame_data.get("loopIteration").copied().unwrap_or(0) as u32;

                                let is_valid_frame =
                                    current_time > 0 && (current_loop > 0 || current_time > 1000);

                                if is_valid_frame {
                                    parsing_success = true;
                                    stats.p_frames += 1;

                                    if debug && stats.p_frames <= 3 {
                                        println!(
                                            "DEBUG: Accepted P-frame - time:{}, loop:{}",
                                            current_time, current_loop
                                        );
                                    }
                                } else if debug && stats.p_frames < 5 {
                                    println!(
                                        "DEBUG: Rejected P-frame - time:{}, loop:{} (invalid)",
                                        current_time, current_loop
                                    );
                                }
                            }
                        } else {
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.failed_frames += 1;
                        }
                    }
                    'S' => {
                        if debug && stats.s_frames < 5 {
                            println!(
                                "DEBUG: Found S-frame, header.s_frame_def.count={}",
                                header.s_frame_def.count
                            );
                        }
                        if header.s_frame_def.count > 0 {
                            if let Ok(data) = parse_s_frame(&mut stream, &header.s_frame_def, debug)
                            {
                                if debug && stats.s_frames < 3 {
                                    println!("DEBUG: Processing S-frame with data: {data:?}");
                                }

                                for (key, value) in &data {
                                    last_slow_data.insert(key.clone(), *value);
                                }

                                if debug && stats.s_frames < 3 {
                                    println!(
                                        "DEBUG: S-frame data updated lastSlow: {last_slow_data:?}"
                                    );
                                }

                                stats.s_frames += 1;

                                if debug && stats.s_frames <= 3 {
                                    println!("DEBUG: S-frame count incremented to {} (data merged into lastSlow)", stats.s_frames);
                                }
                            } else if debug && stats.s_frames < 5 {
                                println!("DEBUG: S-frame parsing failed");
                            }
                        } else if debug && stats.s_frames < 5 {
                            println!("DEBUG: Skipping S-frame - header.s_frame_def.count is 0");
                        }
                    }
                    'H' => {
                        if header.h_frame_def.count > 0 {
                            if let Ok(data) = parse_h_frame(&mut stream, &header.h_frame_def, debug)
                            {
                                frame_data = data.clone();
                                parsing_success = true;
                                stats.h_frames += 1;

                                // Extract GPS home coordinates for GPX export if enabled
                                if export_options.gpx {
                                    let timestamp = last_main_frame_timestamp;

                                    if let (Some(&home_lat_raw), Some(&home_lon_raw)) = (
                                        frame_data.get("GPS_home[0]"),
                                        frame_data.get("GPS_home[1]"),
                                    ) {
                                        if debug && home_coordinates.is_empty() {
                                            println!("DEBUG: HOME raw values - home_lat_raw: {}, home_lon_raw: {}", home_lat_raw, home_lon_raw);
                                            println!(
                                                "DEBUG: HOME converted - lat: {:.7}, lon: {:.7}",
                                                convert_gps_coordinate(home_lat_raw),
                                                convert_gps_coordinate(home_lon_raw)
                                            );
                                        }

                                        let home_coordinate = GpsHomeCoordinate {
                                            home_latitude: convert_gps_coordinate(home_lat_raw),
                                            home_longitude: convert_gps_coordinate(home_lon_raw),
                                            timestamp_us: timestamp,
                                        };
                                        home_coordinates.push(home_coordinate);
                                    }
                                }
                            }
                        } else {
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.h_frames += 1;
                            parsing_success = true;
                        }
                    }
                    'G' => {
                        if header.g_frame_def.count > 0 {
                            // Initialize GPS frame history if needed
                            if gps_frame_history.is_empty() {
                                gps_frame_history = vec![0i32; header.g_frame_def.count];
                            }

                            let mut g_frame_values = vec![0i32; header.g_frame_def.count];

                            if parse_frame_data(
                                &mut stream,
                                &header.g_frame_def,
                                &mut g_frame_values,
                                Some(&gps_frame_history),
                                None,
                                0,
                                false,
                                header.data_version,
                                &header.sysconfig,
                                debug,
                            )
                            .is_ok()
                            {
                                // Update GPS frame history
                                gps_frame_history.copy_from_slice(&g_frame_values);

                                // Copy GPS frame data to output
                                for (i, field_name) in
                                    header.g_frame_def.field_names.iter().enumerate()
                                {
                                    if i < g_frame_values.len() {
                                        let value = g_frame_values[i];
                                        frame_data.insert(field_name.clone(), value);
                                    }
                                }

                                parsing_success = true;
                                stats.g_frames += 1;

                                // Extract GPS coordinates for GPX export if enabled
                                if export_options.gpx {
                                    let gps_time =
                                        frame_data.get("time").copied().unwrap_or(0) as u64;
                                    let timestamp = if gps_time > 0 {
                                        gps_time
                                    } else {
                                        last_main_frame_timestamp
                                    };

                                    if let (Some(&lat_raw), Some(&lon_raw), Some(&alt_raw)) = (
                                        frame_data.get("GPS_coord[0]"),
                                        frame_data.get("GPS_coord[1]"),
                                        frame_data.get("GPS_altitude"),
                                    ) {
                                        let actual_lat =
                                            if let Some(home_coord) = home_coordinates.first() {
                                                home_coord.home_latitude
                                                    + convert_gps_coordinate(lat_raw)
                                            } else {
                                                convert_gps_coordinate(lat_raw)
                                            };

                                        let actual_lon =
                                            if let Some(home_coord) = home_coordinates.first() {
                                                home_coord.home_longitude
                                                    + convert_gps_coordinate(lon_raw)
                                            } else {
                                                convert_gps_coordinate(lon_raw)
                                            };

                                        if debug && gps_coordinates.len() < 3 {
                                            println!("DEBUG: GPS raw values - lat_raw: {}, lon_raw: {}, alt_raw: {}", lat_raw, lon_raw, alt_raw);
                                            println!("DEBUG: GPS converted - lat: {:.7}, lon: {:.7}, alt: {:.2}", 
                                                   actual_lat, actual_lon,
                                                   convert_gps_altitude(alt_raw, &header.firmware_revision));
                                        }

                                        let coordinate = GpsCoordinate {
                                            latitude: actual_lat,
                                            longitude: actual_lon,
                                            altitude: convert_gps_altitude(
                                                alt_raw,
                                                &header.firmware_revision,
                                            ),
                                            timestamp_us: timestamp,
                                            num_sats: frame_data.get("GPS_numSat").copied(),
                                            speed: frame_data
                                                .get("GPS_speed")
                                                .map(|&s| convert_gps_speed(s)),
                                            ground_course: frame_data
                                                .get("GPS_ground_course")
                                                .map(|&c| convert_gps_course(c)),
                                        };
                                        gps_coordinates.push(coordinate);
                                    }
                                }
                            }
                        } else {
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.g_frames += 1;
                            parsing_success = true;
                        }
                    }
                    'E' => {
                        if let Ok(mut event_frame) = parse_e_frame(&mut stream, debug) {
                            frame_data
                                .insert("event_type".to_string(), event_frame.event_type as i32);
                            frame_data.insert("event_description".to_string(), 0);
                            parsing_success = true;
                            stats.e_frames += 1;

                            // Collect event frames for JSON export if enabled
                            if export_options.event {
                                event_frame.timestamp_us = last_main_frame_timestamp;
                                event_frames.push(event_frame);
                            }

                            if debug && stats.e_frames <= 3 {
                                println!(
                                    "DEBUG: Parsed E-frame - Type: {}",
                                    frame_data.get("event_type").unwrap_or(&0)
                                );
                            }
                        } else {
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.e_frames += 1;
                            parsing_success = true;
                        }
                    }
                    _ => {}
                };

                if !parsing_success {
                    stats.failed_frames += 1;
                }

                stats.total_frames += 1;

                // Show progress for large files
                if (debug && stats.total_frames % 50000 == 0) || stats.total_frames % 100000 == 0 {
                    println!("Parsed {} frames so far...", stats.total_frames);
                    std::io::stdout().flush().unwrap_or_default();
                }

                // Store ALL successfully parsed frames
                if parsing_success {
                    let timestamp_us = frame_data.get("time").copied().unwrap_or(0) as u64;
                    let loop_iteration =
                        frame_data.get("loopIteration").copied().unwrap_or(0) as u32;

                    // Update last timestamp for main frames (I, P)
                    if (frame_type == 'I' || frame_type == 'P') && timestamp_us > 0 {
                        last_main_frame_timestamp = timestamp_us;
                    }

                    // S frames inherit timestamp from last main frame
                    let final_timestamp = if frame_type == 'S' && timestamp_us == 0 {
                        last_main_frame_timestamp
                    } else {
                        timestamp_us
                    };

                    if debug && (frame_type == 'I' || frame_type == 'P') && frames.len() < 3 {
                        println!(
                            "DEBUG: Frame {:?} has timestamp {}. Available fields: {:?}",
                            frame_type,
                            timestamp_us,
                            frame_data.keys().collect::<Vec<_>>()
                        );
                        if let Some(time_val) = frame_data.get("time") {
                            println!("DEBUG: 'time' field value: {time_val}");
                        }
                        if let Some(loop_val) = frame_data.get("loopIteration") {
                            println!("DEBUG: 'loopIteration' field value: {loop_val}");
                        }
                    }

                    let decoded_frame = DecodedFrame {
                        frame_type,
                        timestamp_us: final_timestamp,
                        loop_iteration,
                        data: frame_data.clone(),
                    };
                    frames.push(decoded_frame.clone());

                    // Also store in debug_frames for debug purposes
                    if debug {
                        let debug_frame_list = debug_frames.entry(frame_type).or_default();
                        debug_frame_list.push(decoded_frame);
                    }
                }

                // Update timing from first and last valid frames with time data
                if parsing_success {
                    if let Some(time_us) = frame_data.get("time") {
                        let time_val = *time_us as u64;
                        if stats.start_time_us == 0 {
                            stats.start_time_us = time_val;
                        }
                        stats.end_time_us = time_val;
                    }
                }
            }
            Err(_) => break,
        }

        // Safety limits to prevent hanging
        if stats.total_frames > 1000000 || stats.failed_frames > 10000 {
            if debug {
                println!("Hit safety limit - stopping frame parsing");
            }
            break;
        }
    }

    stats.total_bytes = binary_data.len() as u64;

    if debug {
        println!(
            "Parsed {} frames: {} I, {} P, {} H, {} G, {} E, {} S",
            stats.total_frames,
            stats.i_frames,
            stats.p_frames,
            stats.h_frames,
            stats.g_frames,
            stats.e_frames,
            stats.s_frames
        );
        println!("Failed to parse: {} frames", stats.failed_frames);
    }

    Ok((
        stats,
        frames,
        Some(debug_frames),
        gps_coordinates,
        home_coordinates,
        event_frames,
    ))
}

/// Parse frame data using the specified frame definition
#[allow(clippy::too_many_arguments)]
pub fn parse_frame_data(
    stream: &mut BBLDataStream,
    frame_def: &FrameDefinition,
    current_frame: &mut [i32],
    previous_frame: Option<&[i32]>,
    previous2_frame: Option<&[i32]>,
    skipped_frames: u32,
    raw: bool,
    _data_version: u8,
    sysconfig: &HashMap<String, i32>,
    debug: bool,
) -> Result<()> {
    let mut i = 0;
    let mut values = [0i32; 8];

    while i < frame_def.fields.len() {
        let field = &frame_def.fields[i];

        if field.predictor == PREDICT_INC {
            current_frame[i] = apply_predictor_with_debug(
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
                    current_frame[i + j] = apply_predictor_with_debug(
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
                    current_frame[i + j] = apply_predictor_with_debug(
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
                // Count how many consecutive fields use this encoding
                let mut group_count = 1;
                for j in i + 1..i + 8.min(frame_def.fields.len() - i) {
                    if frame_def.fields[j].encoding != ENCODING_TAG8_8SVB {
                        break;
                    }
                    group_count += 1;
                }

                stream.read_tag8_8svb_counted(&mut values, group_count)?;

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
                    current_frame[i + j] = apply_predictor_with_debug(
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
                decode_field_value(stream, field.encoding, &mut values, 0)?;
                let raw_value = values[0];
                let predictor = if raw { PREDICT_0 } else { field.predictor };
                current_frame[i] = apply_predictor_with_debug(
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

/// Parse S-frame (Slow/periodic data) from the stream
///
/// S-frames contain slowly-changing data that is logged less frequently.
/// This function handles all standard encodings including TAG2_3S32 which
/// reads 3 values at once.
pub fn parse_s_frame(
    stream: &mut BBLDataStream,
    frame_def: &FrameDefinition,
    debug: bool,
) -> Result<HashMap<String, i32>> {
    let mut data = HashMap::new();
    let mut field_index = 0;

    while field_index < frame_def.fields.len() {
        let field = &frame_def.fields[field_index];

        match field.encoding {
            ENCODING_SIGNED_VB => {
                let value = stream.read_signed_vb()?;
                data.insert(field.name.clone(), value);
                field_index += 1;
            }
            ENCODING_UNSIGNED_VB => {
                let value = stream.read_unsigned_vb()? as i32;
                data.insert(field.name.clone(), value);
                field_index += 1;
            }
            ENCODING_NEG_14BIT => {
                let value = stream.read_neg_14bit()?;
                data.insert(field.name.clone(), value);
                field_index += 1;
            }
            ENCODING_TAG2_3S32 => {
                // This encoding handles 3 fields at once
                let mut values = [0i32; 8];
                stream.read_tag2_3s32(&mut values)?;

                #[allow(clippy::needless_range_loop)]
                for j in 0..3 {
                    if field_index + j < frame_def.fields.len() {
                        let current_field = &frame_def.fields[field_index + j];
                        data.insert(current_field.name.clone(), values[j]);
                    }
                }
                field_index += 3;
            }
            ENCODING_NULL => {
                data.insert(field.name.clone(), 0);
                field_index += 1;
            }
            _ => {
                if debug {
                    println!(
                        "Unsupported S-frame encoding {} for field {}",
                        field.encoding, field.name
                    );
                }
                // For unsupported encodings, try to read as signed VB
                let value = stream.read_signed_vb().unwrap_or(0);
                data.insert(field.name.clone(), value);
                field_index += 1;
            }
        }
    }

    Ok(data)
}

fn skip_frame(stream: &mut BBLDataStream, frame_type: char, debug: bool) -> Result<()> {
    if debug {
        println!("Skipping {} frame", frame_type);
    }

    // Skip frame by reading a few bytes - this is a simple heuristic
    match frame_type {
        'E' => {
            // Event frames - read event type and some data
            let _event_type = stream.read_byte()?;
            // Read up to 16 bytes of event data
            for _ in 0..16 {
                if stream.eof {
                    break;
                }
                let _ = stream.read_byte();
            }
        }
        'G' | 'H' => {
            // GPS frames - read several fields
            for _ in 0..7 {
                if stream.eof {
                    break;
                }
                let _ = stream.read_unsigned_vb();
            }
        }
        _ => {
            // Unknown frame type - read a few bytes
            for _ in 0..8 {
                if stream.eof {
                    break;
                }
                let _ = stream.read_byte();
            }
        }
    }

    Ok(())
}
