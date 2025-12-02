use crate::types::frame::FrameDefinition;
use std::collections::HashMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// BBL header information
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BBLHeader {
    pub firmware_revision: String,
    pub board_info: String,
    pub craft_name: String,
    pub data_version: u8,
    pub looptime: u32,
    /// Log start datetime from header (ISO 8601 format, e.g., "2024-10-10T18:37:25.559+00:00")
    /// This is used for generating absolute timestamps in GPX exports.
    /// Will be None if not present, or Some("0000-01-01T00:00:00.000+00:00") if clock wasn't set.
    pub log_start_datetime: Option<String>,
    pub i_frame_def: FrameDefinition,
    pub p_frame_def: FrameDefinition,
    pub s_frame_def: FrameDefinition,
    pub g_frame_def: FrameDefinition,
    pub h_frame_def: FrameDefinition,
    pub sysconfig: HashMap<String, i32>,
    pub all_headers: Vec<String>,
}

impl Default for BBLHeader {
    fn default() -> Self {
        Self {
            firmware_revision: String::new(),
            board_info: String::new(),
            craft_name: String::new(),
            data_version: 2,
            looptime: 0,
            log_start_datetime: None,
            i_frame_def: FrameDefinition::new(),
            p_frame_def: FrameDefinition::new(),
            s_frame_def: FrameDefinition::new(),
            g_frame_def: FrameDefinition::new(),
            h_frame_def: FrameDefinition::new(),
            sysconfig: HashMap::new(),
            all_headers: Vec::new(),
        }
    }
}
