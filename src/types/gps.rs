use std::collections::HashMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// GPS coordinate data from G frames
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GpsCoordinate {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
    pub timestamp_us: u64,
    pub num_sats: Option<i32>,
    pub speed: Option<f64>,
    pub ground_course: Option<f64>,
}

/// GPS home coordinate data from H frames
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GpsHomeCoordinate {
    pub home_latitude: f64,
    pub home_longitude: f64,
    pub timestamp_us: u64,
}

/// Event frame data from E frames
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EventFrame {
    pub timestamp_us: u64,
    pub event_type: u8,
    pub event_data: Vec<u8>,
    pub event_description: String,
}
