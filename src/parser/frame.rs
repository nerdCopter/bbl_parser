use crate::error::Result;
use crate::parser::{decoder::*, stream::BBLDataStream};
use crate::types::{
    DecodedFrame, EventFrame, FrameDefinition, FrameHistory, FrameStats, GpsCoordinate,
    GpsHomeCoordinate,
};
use std::collections::HashMap;

// Import GPS/Event parsing functions
use super::decoder::{parse_e_frame, parse_h_frame};

/// Parse frames from binary data
#[allow(clippy::type_complexity)]
pub fn parse_frames(
    binary_data: &[u8],
    header: &crate::types::BBLHeader,
    debug: bool,
) -> Result<(
    FrameStats,
    Vec<DecodedFrame>,
    Option<HashMap<char, Vec<DecodedFrame>>>,
    Vec<GpsCoordinate>,
    Vec<GpsHomeCoordinate>,
    Vec<EventFrame>,
)> {
    let mut stats = FrameStats::default();
    let mut sample_frames = Vec::new();
    let mut debug_frames: Option<HashMap<char, Vec<DecodedFrame>>> =
        if debug { Some(HashMap::new()) } else { None };

    // Collections for GPS and Event export
    let mut gps_coordinates: Vec<GpsCoordinate> = Vec::new();
    let mut home_coordinates: Vec<GpsHomeCoordinate> = Vec::new();
    let mut event_frames: Vec<EventFrame> = Vec::new();

    // GPS frame history for differential encoding
    let mut gps_frame_history: Vec<i32> = Vec::new();

    // Track last main frame timestamp for GPS/Event frames
    let mut last_main_frame_timestamp = 0u64;

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
            sample_frames,
            debug_frames,
            gps_coordinates,
            home_coordinates,
            event_frames,
        ));
    }

    // Initialize frame history for proper P-frame parsing
    let mut frame_history = FrameHistory::new(header.i_frame_def.count);
    let mut stream = BBLDataStream::new(binary_data);

    // Track the most recent S-frame data for merging (following JavaScript approach)
    let mut last_slow_data: HashMap<String, i32> = HashMap::new();

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
                    println!(
                        "Found frame type '{}' at offset {}",
                        frame_type, frame_start_pos
                    );
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
                            )
                            .is_ok()
                            {
                                // Copy parsed data to frame_data HashMap
                                for (i, field_name) in
                                    header.i_frame_def.field_names.iter().enumerate()
                                {
                                    if i < frame_history.current_frame.len() {
                                        frame_data.insert(
                                            field_name.clone(),
                                            frame_history.current_frame[i],
                                        );
                                    }
                                }

                                // Merge lastSlow data into I-frame (following JavaScript approach)
                                for (key, value) in &last_slow_data {
                                    frame_data.insert(key.clone(), *value);
                                }

                                if debug && stats.i_frames <= 2 {
                                    println!("DEBUG: I-frame merged lastSlow. rxSignalReceived: {:?}, rxFlightChannelsValid: {:?}", 
                                             frame_data.get("rxSignalReceived"), frame_data.get("rxFlightChannelsValid"));
                                }

                                // Update history for future P-frames
                                frame_history.update(frame_history.current_frame.clone());

                                // Update last main frame timestamp
                                if let Some(&time) = frame_data.get("time") {
                                    last_main_frame_timestamp = time as u64;
                                }

                                parsing_success = true;
                                stats.i_frames += 1;
                            }
                        }
                    }
                    'P' => {
                        if header.p_frame_def.count > 0 && frame_history.valid {
                            frame_history.current_frame.fill(0);

                            if parse_frame_data(
                                &mut stream,
                                &header.p_frame_def,
                                &mut frame_history.current_frame,
                                Some(&frame_history.previous_frame),
                                Some(&frame_history.previous2_frame),
                                0,     // TODO: Calculate skipped frames properly
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                            )
                            .is_ok()
                            {
                                // Copy parsed data using I-frame field names (P-frames use I-frame structure)
                                for (i, field_name) in
                                    header.i_frame_def.field_names.iter().enumerate()
                                {
                                    if i < frame_history.current_frame.len() {
                                        frame_data.insert(
                                            field_name.clone(),
                                            frame_history.current_frame[i],
                                        );
                                    }
                                }

                                // Merge lastSlow data into P-frame (following JavaScript approach)
                                for (key, value) in &last_slow_data {
                                    frame_data.insert(key.clone(), *value);
                                }

                                if debug && stats.p_frames <= 2 {
                                    println!("DEBUG: P-frame merged lastSlow. rxSignalReceived: {:?}, rxFlightChannelsValid: {:?}", 
                                             frame_data.get("rxSignalReceived"), frame_data.get("rxFlightChannelsValid"));
                                }

                                // Update history
                                frame_history.update(frame_history.current_frame.clone());

                                // Update last main frame timestamp
                                if let Some(&time) = frame_data.get("time") {
                                    last_main_frame_timestamp = time as u64;
                                }

                                parsing_success = true;
                                stats.p_frames += 1;
                            }
                        } else {
                            // Skip P-frame if we don't have valid I-frame history
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.failed_frames += 1;
                        }
                    }
                    'S' => {
                        if header.s_frame_def.count > 0 {
                            if let Ok(data) = parse_s_frame(&mut stream, &header.s_frame_def, debug)
                            {
                                // Following JavaScript approach: update lastSlow data
                                if debug {
                                    println!("DEBUG: Processing S-frame with data: {:?}", data);
                                }

                                for (key, value) in &data {
                                    last_slow_data.insert(key.clone(), *value);
                                }

                                if debug {
                                    println!(
                                        "DEBUG: S-frame data updated lastSlow: {:?}",
                                        last_slow_data
                                    );
                                }

                                frame_data = data;
                                parsing_success = true;
                                stats.s_frames += 1;
                            }
                        }
                    }
                    'G' | 'H' | 'E' => {
                        match frame_type {
                            'H' => {
                                // Parse H-frame (GPS Home)
                                if header.h_frame_def.count > 0 {
                                    if let Ok(data) =
                                        parse_h_frame(&mut stream, &header.h_frame_def, debug)
                                    {
                                        frame_data = data.clone();
                                        parsing_success = true;
                                        stats.h_frames += 1;

                                        // Extract GPS home coordinates for GPX export
                                        let timestamp = last_main_frame_timestamp;

                                        if let (Some(&home_lat_raw), Some(&home_lon_raw)) = (
                                            frame_data.get("GPS_home[0]"),
                                            frame_data.get("GPS_home[1]"),
                                        ) {
                                            use crate::conversion::convert_gps_coordinate;

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
                                                home_longitude: convert_gps_coordinate(
                                                    home_lon_raw,
                                                ),
                                                timestamp_us: timestamp,
                                            };
                                            home_coordinates.push(home_coordinate);
                                        }
                                    }
                                } else {
                                    skip_frame(&mut stream, frame_type, debug)?;
                                    stats.h_frames += 1;
                                    parsing_success = true;
                                }
                            }
                            'G' => {
                                // Parse G-frame (GPS data)
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
                                        Some(&gps_frame_history), // Use GPS frame history for differential encoding
                                        None,  // GPS frames typically don't use previous2
                                        0,     // TODO: Calculate skipped frames properly
                                        false, // Not raw
                                        header.data_version,
                                        &header.sysconfig,
                                    )
                                    .is_ok()
                                    {
                                        // Update GPS frame history with new values
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

                                        // Extract GPS coordinates for GPX export
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
                                            use crate::conversion::{
                                                convert_gps_altitude, convert_gps_coordinate,
                                                convert_gps_course, convert_gps_speed,
                                            };

                                            // GPS coordinates are deltas from home position
                                            // Need to add home coordinates to get actual GPS position
                                            let actual_lat = if let Some(home_coord) =
                                                home_coordinates.first()
                                            {
                                                home_coord.home_latitude
                                                    + convert_gps_coordinate(lat_raw)
                                            } else {
                                                convert_gps_coordinate(lat_raw)
                                            };

                                            let actual_lon = if let Some(home_coord) =
                                                home_coordinates.first()
                                            {
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
                                } else {
                                    skip_frame(&mut stream, frame_type, debug)?;
                                    stats.g_frames += 1;
                                    parsing_success = true;
                                }
                            }
                            'E' => {
                                // Parse E-frame (Event)
                                if let Ok(mut event_frame) = parse_e_frame(&mut stream, debug) {
                                    // Store event data for potential export
                                    frame_data.insert(
                                        "event_type".to_string(),
                                        event_frame.event_type as i32,
                                    );
                                    parsing_success = true;
                                    stats.e_frames += 1;

                                    // Collect event frames for export
                                    event_frame.timestamp_us = last_main_frame_timestamp;
                                    event_frames.push(event_frame);

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
                        }
                    }
                    _ => {}
                };

                if !parsing_success {
                    stats.failed_frames += 1;
                }

                stats.total_frames += 1;

                // Show progress for large files
                if debug && stats.total_frames % 50000 == 0 || stats.total_frames % 100000 == 0 {
                    println!("Parsed {} frames so far...", stats.total_frames);
                }

                // Store only a few sample frames for display purposes
                if parsing_success && sample_frames.len() < 10 {
                    let decoded_frame = create_decoded_frame(frame_type, &frame_data);
                    sample_frames.push(decoded_frame.clone());

                    // Store debug frames if debug mode is enabled
                    if let Some(ref mut debug_map) = debug_frames {
                        let debug_frame_list = debug_map.entry(frame_type).or_insert_with(Vec::new);
                        debug_frame_list.push(decoded_frame);
                    }
                } else if parsing_success {
                    // Even if we don't store in sample_frames, still store for debug if enabled
                    if let Some(ref mut debug_map) = debug_frames {
                        let debug_frame_list = debug_map.entry(frame_type).or_insert_with(Vec::new);
                        // Store frames strategically for the display pattern (first/middle/last)
                        if debug_frame_list.len() < 50 {
                            let decoded_frame = create_decoded_frame(frame_type, &frame_data);
                            debug_frame_list.push(decoded_frame);
                        }
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

        // More aggressive safety limits to prevent hanging
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
        sample_frames,
        debug_frames,
        gps_coordinates,
        home_coordinates,
        event_frames,
    ))
}

fn create_decoded_frame(frame_type: char, frame_data: &HashMap<String, i32>) -> DecodedFrame {
    let timestamp_us = frame_data.get("time").copied().unwrap_or(0) as u64;
    let loop_iteration = frame_data.get("loopIteration").copied().unwrap_or(0) as u32;

    DecodedFrame {
        frame_type,
        timestamp_us,
        loop_iteration,
        data: frame_data.clone(),
    }
}

/// Parse frame data using the specified frame definition
#[allow(clippy::too_many_arguments)]
pub fn parse_frame_data(
    stream: &mut BBLDataStream,
    frame_def: &FrameDefinition,
    current_frame: &mut [i32],
    previous_frame: Option<&[i32]>,
    previous2_frame: Option<&[i32]>,
    _skipped_frames: u32,
    raw: bool,
    _data_version: u8,
    sysconfig: &HashMap<String, i32>,
) -> Result<()> {
    let mut i = 0;
    let mut values = [0i32; 8];

    while i < frame_def.fields.len() {
        let field = &frame_def.fields[i];

        if field.predictor == PREDICT_INC {
            current_frame[i] = apply_predictor(
                field.predictor,
                0,
                i,
                current_frame,
                previous_frame.unwrap_or(&[]),
                previous2_frame.unwrap_or(&[]),
                sysconfig,
            )?;
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
                    current_frame[i + j] = apply_predictor(
                        predictor,
                        values[j],
                        i + j,
                        current_frame,
                        previous_frame.unwrap_or(&[]),
                        previous2_frame.unwrap_or(&[]),
                        sysconfig,
                    )?;
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
                        predictor,
                        values[j],
                        i + j,
                        current_frame,
                        previous_frame.unwrap_or(&[]),
                        previous2_frame.unwrap_or(&[]),
                        sysconfig,
                    )?;
                }
                i += 3;
                continue;
            }

            ENCODING_TAG8_8SVB => {
                stream.read_tag8_8svb(&mut values)?;

                // Apply predictors for the 8 fields
                for j in 0..8 {
                    if i + j >= frame_def.fields.len() {
                        break;
                    }
                    let predictor = if raw {
                        PREDICT_0
                    } else {
                        frame_def.fields[i + j].predictor
                    };
                    current_frame[i + j] = apply_predictor(
                        predictor,
                        values[j],
                        i + j,
                        current_frame,
                        previous_frame.unwrap_or(&[]),
                        previous2_frame.unwrap_or(&[]),
                        sysconfig,
                    )?;
                }
                i += 8;
                continue;
            }

            _ => {
                decode_field_value(stream, field.encoding, &mut values, 0)?;
                let raw_value = values[0];
                let predictor = if raw { PREDICT_0 } else { field.predictor };
                current_frame[i] = apply_predictor(
                    predictor,
                    raw_value,
                    i,
                    current_frame,
                    previous_frame.unwrap_or(&[]),
                    previous2_frame.unwrap_or(&[]),
                    sysconfig,
                )?;
            }
        }

        i += 1;
    }

    Ok(())
}

fn parse_s_frame(
    stream: &mut BBLDataStream,
    frame_def: &FrameDefinition,
    debug: bool,
) -> Result<HashMap<String, i32>> {
    let mut data = HashMap::new();

    // Parse each field according to the frame definition
    for field in &frame_def.fields {
        let _values = [0i32; 1];
        let value = match field.encoding {
            ENCODING_SIGNED_VB => stream.read_signed_vb()?,
            ENCODING_UNSIGNED_VB => stream.read_unsigned_vb()? as i32,
            ENCODING_NEG_14BIT => stream.read_neg_14bit()?,
            ENCODING_NULL => 0,
            _ => {
                if debug {
                    println!(
                        "Unsupported S-frame encoding {} for field {}",
                        field.encoding, field.name
                    );
                }
                // For unsupported encodings, try to read as signed VB
                stream.read_signed_vb().unwrap_or(0)
            }
        };

        data.insert(field.name.clone(), value);
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
