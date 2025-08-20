use crate::types::{
    BBLHeader, DecodedFrame, EventFrame, FrameStats, GpsCoordinate, GpsHomeCoordinate,
};
use std::collections::HashMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Complete BBL log data
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BBLLog {
    pub log_number: usize,
    pub total_logs: usize,
    pub header: BBLHeader,
    pub stats: FrameStats,
    pub sample_frames: Vec<DecodedFrame>,
    pub debug_frames: Option<HashMap<char, Vec<DecodedFrame>>>,
    pub gps_coordinates: Vec<GpsCoordinate>,
    pub home_coordinates: Vec<GpsHomeCoordinate>,
    pub event_frames: Vec<EventFrame>,
}

impl BBLLog {
    pub fn new(log_number: usize, total_logs: usize) -> Self {
        Self {
            log_number,
            total_logs,
            header: BBLHeader::default(),
            stats: FrameStats::default(),
            sample_frames: Vec::new(),
            debug_frames: None,
            gps_coordinates: Vec::new(),
            home_coordinates: Vec::new(),
            event_frames: Vec::new(),
        }
    }

    /// Get the duration of the log in microseconds
    pub fn duration_us(&self) -> u64 {
        self.stats
            .end_time_us
            .saturating_sub(self.stats.start_time_us)
    }

    /// Get the duration of the log in seconds
    pub fn duration_seconds(&self) -> f64 {
        self.duration_us() as f64 / 1_000_000.0
    }

    /// Check if this log contains GPS data
    pub fn has_gps_data(&self) -> bool {
        self.stats.g_frames > 0
    }

    /// Check if this log contains slow frames
    pub fn has_slow_data(&self) -> bool {
        self.stats.s_frames > 0
    }

    /// Get frames of a specific type
    pub fn get_frames_by_type(&self, frame_type: char) -> Option<&Vec<DecodedFrame>> {
        self.debug_frames.as_ref()?.get(&frame_type)
    }
}

/// Container for multiple BBL logs from a single file
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BBLFile {
    pub logs: Vec<BBLLog>,
    pub filename: String,
}

impl BBLFile {
    pub fn new(filename: String) -> Self {
        Self {
            logs: Vec::new(),
            filename,
        }
    }

    pub fn add_log(&mut self, log: BBLLog) {
        self.logs.push(log);
    }

    /// Get total number of logs in the file
    pub fn log_count(&self) -> usize {
        self.logs.len()
    }

    /// Get total duration of all logs in seconds
    pub fn total_duration_seconds(&self) -> f64 {
        self.logs.iter().map(|log| log.duration_seconds()).sum()
    }

    /// Check if any log contains GPS data
    pub fn has_gps_data(&self) -> bool {
        self.logs.iter().any(|log| log.has_gps_data())
    }
}
