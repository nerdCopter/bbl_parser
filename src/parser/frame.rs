use std::collections::HashMap;
use crate::error::{BBLError, Result};
use crate::types::{FrameDefinition, DecodedFrame, FrameHistory, FrameStats};
use crate::parser::{stream::BBLDataStream, decoder::*};

/// Parse frames from binary data
pub fn parse_frames(
    binary_data: &[u8], 
    header: &crate::types::BBLHeader, 
    debug: bool
) -> Result<(FrameStats, Vec<DecodedFrame>, Option<HashMap<char, Vec<DecodedFrame>>>)> {
    let mut stats = FrameStats::default();
    let mut sample_frames = Vec::new();
    let mut debug_frames: Option<HashMap<char, Vec<DecodedFrame>>> = if debug {
        Some(HashMap::new())
    } else {
        None
    };
    
    if debug {
        println!("Binary data size: {} bytes", binary_data.len());
        if !binary_data.is_empty() {
            println!("First 16 bytes: {:02X?}", &binary_data[..16.min(binary_data.len())]);
        }
    }
    
    if binary_data.is_empty() {
        return Ok((stats, sample_frames, debug_frames));
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
                            println!("Unknown frame type byte 0x{:02X} ('{:?}') at offset {}", 
                                   frame_type_byte, frame_type_byte as char, frame_start_pos);
                        }
                        stats.failed_frames += 1;
                        continue;
                    }
                };
                
                if debug && stats.total_frames < 3 {
                    println!("Found frame type '{}' at offset {}", frame_type, frame_start_pos);
                }
                
                // Parse frame using proper streaming logic
                let mut frame_data = HashMap::new();
                let mut parsing_success = false;
                
                match frame_type {
                    'I' => {
                        if header.i_frame_def.count > 0 {
                            // I-frames reset the prediction history
                            frame_history.current_frame.fill(0);
                            
                            if let Ok(_) = parse_frame_data(
                                &mut stream,
                                &header.i_frame_def,
                                &mut frame_history.current_frame,
                                None, // I-frames don't use prediction
                                None,
                                0,
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                            ) {
                                // Copy parsed data to frame_data HashMap
                                for (i, field_name) in header.i_frame_def.field_names.iter().enumerate() {
                                    if i < frame_history.current_frame.len() {
                                        frame_data.insert(field_name.clone(), frame_history.current_frame[i]);
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
                                parsing_success = true;
                                stats.i_frames += 1;
                            }
                        }
                    },
                    'P' => {
                        if header.p_frame_def.count > 0 && frame_history.valid {
                            frame_history.current_frame.fill(0);
                            
                            if let Ok(_) = parse_frame_data(
                                &mut stream,
                                &header.p_frame_def,
                                &mut frame_history.current_frame,
                                Some(&frame_history.previous_frame),
                                Some(&frame_history.previous2_frame),
                                0, // TODO: Calculate skipped frames properly
                                false, // Not raw
                                header.data_version,
                                &header.sysconfig,
                            ) {
                                // Copy parsed data using I-frame field names (P-frames use I-frame structure)
                                for (i, field_name) in header.i_frame_def.field_names.iter().enumerate() {
                                    if i < frame_history.current_frame.len() {
                                        frame_data.insert(field_name.clone(), frame_history.current_frame[i]);
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
                                parsing_success = true;
                                stats.p_frames += 1;
                            }
                        } else {
                            // Skip P-frame if we don't have valid I-frame history
                            skip_frame(&mut stream, frame_type, debug)?;
                            stats.failed_frames += 1;
                        }
                    },
                    'S' => {
                        if header.s_frame_def.count > 0 {
                            if let Ok(data) = parse_s_frame(&mut stream, &header.s_frame_def, debug) {
                                // Following JavaScript approach: update lastSlow data
                                if debug {
                                    println!("DEBUG: Processing S-frame with data: {:?}", data);
                                }
                                
                                for (key, value) in &data {
                                    last_slow_data.insert(key.clone(), *value);
                                }
                                
                                if debug {
                                    println!("DEBUG: S-frame data updated lastSlow: {:?}", last_slow_data);
                                }
                                
                                frame_data = data;
                                parsing_success = true;
                                stats.s_frames += 1;
                            }
                        }
                    },
                    'G' | 'H' | 'E' => {
                        skip_frame(&mut stream, frame_type, debug)?;
                        match frame_type {
                            'G' => stats.g_frames += 1,
                            'H' => stats.h_frames += 1,
                            'E' => stats.e_frames += 1,
                            _ => {}
                        }
                        parsing_success = true;
                    },
                    _ => {}
                };
                
                if !parsing_success {
                    stats.failed_frames += 1;
                }
                
                stats.total_frames += 1;
                
                // Show progress for large files  
                if debug && stats.total_frames % 50000 == 0 {
                    println!("Parsed {} frames so far...", stats.total_frames);
                } else if stats.total_frames % 100000 == 0 {
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
        println!("Parsed {} frames: {} I, {} P, {} H, {} G, {} E, {} S",
                 stats.total_frames, stats.i_frames, stats.p_frames,
                 stats.h_frames, stats.g_frames, stats.e_frames, stats.s_frames);
        println!("Failed to parse: {} frames", stats.failed_frames);
    }
    
    Ok((stats, sample_frames, debug_frames))
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
pub fn parse_frame_data(
    stream: &mut BBLDataStream,
    frame_def: &FrameDefinition,
    current_frame: &mut [i32],
    previous_frame: Option<&[i32]>,
    previous2_frame: Option<&[i32]>,
    skipped_frames: u32,
    raw: bool,
    data_version: u8,
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
                previous_frame,
                previous2_frame,
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
                    let predictor = if raw { PREDICT_0 } else { frame_def.fields[i + j].predictor };
                    current_frame[i + j] = apply_predictor(
                        predictor,
                        values[j],
                        i + j,
                        current_frame,
                        previous_frame,
                        previous2_frame,
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
                    let predictor = if raw { PREDICT_0 } else { frame_def.fields[i + j].predictor };
                    current_frame[i + j] = apply_predictor(
                        predictor,
                        values[j],
                        i + j,
                        current_frame,
                        previous_frame,
                        previous2_frame,
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
                    let predictor = if raw { PREDICT_0 } else { frame_def.fields[i + j].predictor };
                    current_frame[i + j] = apply_predictor(
                        predictor,
                        values[j],
                        i + j,
                        current_frame,
                        previous_frame,
                        previous2_frame,
                        sysconfig,
                    )?;
                }
                i += 8;
                continue;
            }
            
            _ => {
                let raw_value = decode_field_value(stream, field.encoding, &mut values, 0)?;
                let predictor = if raw { PREDICT_0 } else { field.predictor };
                current_frame[i] = apply_predictor(
                    predictor,
                    raw_value,
                    i,
                    current_frame,
                    previous_frame,
                    previous2_frame,
                    sysconfig,
                )?;
            }
        }
        
        i += 1;
    }

    Ok(())
}

fn parse_s_frame(stream: &mut BBLDataStream, frame_def: &FrameDefinition, debug: bool) -> Result<HashMap<String, i32>> {
    let mut data = HashMap::new();
    
    // Parse each field according to the frame definition
    for field in &frame_def.fields {
        let mut values = [0i32; 1];
        let value = match field.encoding {
            ENCODING_SIGNED_VB => stream.read_signed_vb()?,
            ENCODING_UNSIGNED_VB => stream.read_unsigned_vb()? as i32,
            ENCODING_NEG_14BIT => stream.read_neg_14bit()?,
            ENCODING_NULL => 0,
            _ => {
                if debug {
                    println!("Unsupported S-frame encoding {} for field {}", field.encoding, field.name);
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
                if stream.eof { break; }
                let _ = stream.read_byte();
            }
        },
        'G' | 'H' => {
            // GPS frames - read several fields
            for _ in 0..7 {
                if stream.eof { break; }
                let _ = stream.read_unsigned_vb();
            }
        },
        _ => {
            // Unknown frame type - read a few bytes
            for _ in 0..8 {
                if stream.eof { break; }
                let _ = stream.read_byte();
            }
        }
    }
    
    Ok(())
}
