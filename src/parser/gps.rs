//! GPS frame parsing helper module
//!
//! Contains functions for parsing G-frames (GPS data) and H-frames (GPS home position)
//! from blackbox log data. These helpers are used by both the library parser and CLI binary.

use crate::conversion::{
    convert_gps_altitude, convert_gps_coordinate, convert_gps_course, convert_gps_speed,
};
use crate::parser::decoder::{
    ENCODING_NEG_14BIT, ENCODING_NULL, ENCODING_SIGNED_VB, ENCODING_UNSIGNED_VB,
};
use crate::parser::frame::parse_frame_data;
use crate::parser::stream::BBLDataStream;
use crate::types::{FrameDefinition, GpsCoordinate, GpsHomeCoordinate};
use crate::Result;
use std::collections::HashMap;

/// Parse H-frame (GPS home position) data from the stream
///
/// H-frames contain the GPS home position that serves as the reference point
/// for all subsequent G-frame GPS coordinates.
pub fn parse_h_frame(
    stream: &mut BBLDataStream,
    frame_def: &FrameDefinition,
    debug: bool,
) -> Result<HashMap<String, i32>> {
    let mut data = HashMap::new();

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
                // Unsupported H-frame encoding - return error instead of silently continuing
                // This prevents stream desynchronization from being masked by default values
                return Err(anyhow::anyhow!(
                    "Unsupported H-frame encoding {} for field '{}' - stream desynchronization possible",
                    field.encoding, field.name
                ));
            }
        };

        data.insert(field.name.clone(), value);
    }

    Ok(data)
}

/// Extract GPS home coordinate from parsed H-frame data
///
/// Converts raw H-frame field values to a `GpsHomeCoordinate` struct.
pub fn extract_home_coordinate(
    frame_data: &HashMap<String, i32>,
    timestamp_us: u64,
    debug: bool,
) -> Option<GpsHomeCoordinate> {
    if let (Some(&home_lat_raw), Some(&home_lon_raw)) =
        (frame_data.get("GPS_home[0]"), frame_data.get("GPS_home[1]"))
    {
        if debug {
            println!(
                "DEBUG: HOME raw values - home_lat_raw: {}, home_lon_raw: {}",
                home_lat_raw, home_lon_raw
            );
            println!(
                "DEBUG: HOME converted - lat: {:.7}, lon: {:.7}",
                convert_gps_coordinate(home_lat_raw),
                convert_gps_coordinate(home_lon_raw)
            );
        }

        Some(GpsHomeCoordinate {
            home_latitude: convert_gps_coordinate(home_lat_raw),
            home_longitude: convert_gps_coordinate(home_lon_raw),
            timestamp_us,
        })
    } else {
        None
    }
}

/// Parse G-frame (GPS position) data using differential encoding
///
/// G-frames use differential encoding similar to P-frames, where values are
/// encoded as deltas from the previous G-frame. This function properly decodes
/// the G-frame using the GPS frame history for prediction.
#[allow(clippy::too_many_arguments)]
pub fn parse_g_frame(
    stream: &mut BBLDataStream,
    frame_def: &FrameDefinition,
    gps_frame_history: &mut Vec<i32>,
    data_version: u8,
    sysconfig: &HashMap<String, i32>,
    debug: bool,
) -> Result<HashMap<String, i32>> {
    if debug {
        println!("Parsing G frame with {} fields", frame_def.count);
    }

    // Initialize or resize GPS frame history if needed
    // This prevents panic in copy_from_slice if caller passes pre-populated history with wrong length
    if gps_frame_history.len() != frame_def.count {
        gps_frame_history.resize(frame_def.count, 0);
    }

    let mut g_frame_values = vec![0i32; frame_def.count];

    parse_frame_data(
        stream,
        frame_def,
        &mut g_frame_values,
        Some(gps_frame_history), // Use GPS frame history for differential encoding
        None,                    // GPS frames typically don't use previous2
        0,                       // skipped frames
        false,                   // Not raw
        data_version,
        sysconfig,
    )?;

    // Update GPS frame history with new values
    gps_frame_history.copy_from_slice(&g_frame_values);

    // Build output HashMap
    let mut frame_data = HashMap::new();
    for (i, field_name) in frame_def.field_names.iter().enumerate() {
        if i < g_frame_values.len() {
            frame_data.insert(field_name.clone(), g_frame_values[i]);
        }
    }

    Ok(frame_data)
}

/// Extract GPS coordinate from parsed G-frame data
///
/// Converts raw G-frame field values to a `GpsCoordinate` struct,
/// applying the home coordinate offset if available.
#[allow(clippy::too_many_arguments)]
pub fn extract_gps_coordinate(
    frame_data: &HashMap<String, i32>,
    home_coordinates: &[GpsHomeCoordinate],
    timestamp_us: u64,
    firmware_revision: &str,
    debug: bool,
) -> Option<GpsCoordinate> {
    if let (Some(&lat_raw), Some(&lon_raw), Some(&alt_raw)) = (
        frame_data.get("GPS_coord[0]"),
        frame_data.get("GPS_coord[1]"),
        frame_data.get("GPS_altitude"),
    ) {
        // GPS coordinates are deltas from home position
        // Need to add home coordinates to get actual GPS position
        let (home_lat, home_lon) = home_coordinates
            .first()
            .map(|h| (h.home_latitude, h.home_longitude))
            .unwrap_or((0.0, 0.0));

        let actual_lat = home_lat + convert_gps_coordinate(lat_raw);
        let actual_lon = home_lon + convert_gps_coordinate(lon_raw);

        if debug {
            println!(
                "DEBUG: GPS raw values - lat_raw: {}, lon_raw: {}, alt_raw: {}",
                lat_raw, lon_raw, alt_raw
            );
            println!(
                "DEBUG: GPS converted - lat: {:.7}, lon: {:.7}, alt: {:.2}",
                actual_lat,
                actual_lon,
                convert_gps_altitude(alt_raw, firmware_revision)
            );
        }

        Some(GpsCoordinate {
            latitude: actual_lat,
            longitude: actual_lon,
            altitude: convert_gps_altitude(alt_raw, firmware_revision),
            timestamp_us,
            num_sats: frame_data.get("GPS_numSat").copied(),
            speed: frame_data.get("GPS_speed").map(|&s| convert_gps_speed(s)),
            ground_course: frame_data
                .get("GPS_ground_course")
                .map(|&c| convert_gps_course(c)),
        })
    } else {
        None
    }
}
