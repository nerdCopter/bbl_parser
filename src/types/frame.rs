use std::collections::HashMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Field definition for a frame type
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FieldDefinition {
    pub name: String,
    pub signed: bool,
    pub predictor: u8,
    pub encoding: u8,
}

/// Frame definition containing field specifications
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FrameDefinition {
    pub fields: Vec<FieldDefinition>,
    pub field_names: Vec<String>,
    pub count: usize,
}

impl FrameDefinition {
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            field_names: Vec::new(),
            count: 0,
        }
    }

    pub fn from_field_names(names: Vec<String>) -> Self {
        let fields = names
            .iter()
            .map(|name| FieldDefinition {
                name: name.clone(),
                signed: false,
                predictor: 0,
                encoding: 0,
            })
            .collect();
        let count = names.len();
        Self {
            fields,
            field_names: names,
            count,
        }
    }

    pub fn update_signed(&mut self, signed_data: &[bool]) {
        for (i, field) in self.fields.iter_mut().enumerate() {
            if i < signed_data.len() {
                field.signed = signed_data[i];
            }
        }
    }

    pub fn update_predictors(&mut self, predictors: &[u8]) {
        for (i, field) in self.fields.iter_mut().enumerate() {
            if i < predictors.len() {
                field.predictor = predictors[i];
            }
        }
    }

    pub fn update_encoding(&mut self, encodings: &[u8]) {
        for (i, field) in self.fields.iter_mut().enumerate() {
            if i < encodings.len() {
                field.encoding = encodings[i];
            }
        }
    }
}

impl Default for FrameDefinition {
    fn default() -> Self {
        Self::new()
    }
}

/// Decoded frame data
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DecodedFrame {
    pub frame_type: char,
    pub timestamp_us: u64,
    pub loop_iteration: u32,
    pub data: HashMap<String, i32>,
}

/// Frame statistics
#[derive(Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FrameStats {
    pub i_frames: u32,
    pub p_frames: u32,
    pub h_frames: u32,
    pub g_frames: u32,
    pub e_frames: u32,
    pub s_frames: u32,
    pub total_frames: u32,
    pub total_bytes: u64,
    pub start_time_us: u64,
    pub end_time_us: u64,
    pub failed_frames: u32,
    pub missing_iterations: u64,
}

/// Frame history for prediction during parsing
pub struct FrameHistory {
    pub current_frame: Vec<i32>,
    pub previous_frame: Vec<i32>,
    pub previous2_frame: Vec<i32>,
    pub valid: bool,
}

impl FrameHistory {
    pub fn new(field_count: usize) -> Self {
        Self {
            current_frame: vec![0; field_count],
            previous_frame: vec![0; field_count],
            previous2_frame: vec![0; field_count],
            valid: false,
        }
    }

    pub fn update(&mut self, new_frame: Vec<i32>) {
        self.previous2_frame = std::mem::take(&mut self.previous_frame);
        self.previous_frame = std::mem::take(&mut self.current_frame);
        self.current_frame = new_frame;
        self.valid = true;
    }
}
