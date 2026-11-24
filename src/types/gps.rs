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
    pub timestamp_us: u64,   // Time in microseconds
    pub event_type: u8,      // Event type ID
    pub event_data: Vec<u8>, // Raw event data bytes
    pub event_name: String,  // Human-readable event name/description
    /// Decoded payload for the event types blackbox_decode writes to its
    /// `.event` file; `None` for every other event.
    pub data: Option<EventData>,
}

/// New value of an inflight adjustment event
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AdjustmentValue {
    /// Function id 0..=127: signed VB payload
    Int(i32),
    /// Function id above 127: raw 32-bit float payload
    Float(f32),
}

/// Decoded event payload, one variant per event type blackbox_decode exports
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum EventData {
    /// Sync beep; `time_us` is the time carried in the payload
    SyncBeep { time_us: u32 },
    /// Inflight adjustment; `function` is the raw id (bit 7 selects a float value)
    InflightAdjustment {
        function: u8,
        value: AdjustmentValue,
    },
    /// Logging resume; `time_us` is the time carried in the payload
    LoggingResume { log_iteration: u32, time_us: u32 },
    /// Disarm with its reason code
    Disarm { reason: u32 },
    /// Flight mode flags change
    FlightMode { flags: u32, last_flags: u32 },
    /// Valid end of log message
    LogEnd,
}
