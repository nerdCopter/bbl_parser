use std::collections::HashMap;
use crate::error::{BBLError, Result};
use crate::types::{BBLHeader, FrameDefinition, FieldDefinition};

/// Parse BBL headers from text
pub fn parse_headers_from_text(header_text: &str, debug: bool) -> Result<BBLHeader> {
    let mut header = BBLHeader::default();
    
    for line in header_text.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with("H ") {
            continue;
        }
        
        header.all_headers.push(line.to_string());
        
        if debug {
            println!("Processing header: {}", line);
        }
        
        // Parse specific headers following JavaScript reference
        if line.starts_with("H Firmware revision:") {
            header.firmware_revision = line.strip_prefix("H Firmware revision:").unwrap_or("").trim().to_string();
        } else if line.starts_with("H Board information:") {
            header.board_info = line.strip_prefix("H Board information:").unwrap_or("").trim().to_string();
        } else if line.starts_with("H Craft name:") {
            header.craft_name = line.strip_prefix("H Craft name:").unwrap_or("").trim().to_string();
        } else if line.starts_with("H Data version:") {
            if let Ok(version) = line.strip_prefix("H Data version:").unwrap_or("2").trim().parse() {
                header.data_version = version;
            }
        } else if line.starts_with("H looptime:") {
            if let Ok(lt) = line.strip_prefix("H looptime:").unwrap_or("0").trim().parse() {
                header.looptime = lt;
            }
        } else if line.starts_with("H Field I name:") {
            // Parse I frame field names
            if let Some(field_str) = line.strip_prefix("H Field I name:") {
                let names: Vec<String> = field_str.split(',').map(|s| s.trim().to_string()).collect();
                header.i_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field P predictor:") {
            // Parse P frame predictors - P-frames use I-frame field names with different predictors/encodings
            if let Some(predictor_str) = line.strip_prefix("H Field P predictor:") {
                let predictors: Result<Vec<u8>, _> = predictor_str.split(',').map(|s| s.trim().parse()).collect();
                if let Ok(predictors) = predictors {
                    // Create P-frame definition using I-frame field names but P-frame predictors
                    header.p_frame_def = header.i_frame_def.clone();
                    header.p_frame_def.update_predictors(&predictors);
                }
            }
        } else if line.starts_with("H Field P encoding:") {
            // Parse P frame encodings
            if let Some(encoding_str) = line.strip_prefix("H Field P encoding:") {
                let encodings: Result<Vec<u8>, _> = encoding_str.split(',').map(|s| s.trim().parse()).collect();
                if let Ok(encodings) = encodings {
                    header.p_frame_def.update_encoding(&encodings);
                }
            }
        } else if line.starts_with("H Field P name:") {
            // Legacy P frame field names (should not exist in modern logs)
            // **BLACKBOX_DECODE COMPATIBILITY**: P-frame fields must map to I-frame indices
            if let Some(field_str) = line.strip_prefix("H Field P name:") {
                let p_field_names: Vec<String> = field_str.split(',').map(|s| s.trim().to_string()).collect();
                
                // Create P-frame definition with correct field indices matching I-frame positions
                let mut p_frame_def = FrameDefinition::new();
                for p_field_name in p_field_names {
                    // Find the index of this field in the I-frame definition
                    if let Some(i_frame_idx) = header.i_frame_def.field_names.iter().position(|name| name == &p_field_name) {
                        // Create field definition using I-frame index
                        let field = FieldDefinition {
                            name: p_field_name.clone(),
                            signed: false,
                            predictor: 0,
                            encoding: 0,
                        };
                        p_frame_def.fields.push(field);
                        p_frame_def.field_names.push(p_field_name);
                    }
                }
                p_frame_def.count = p_frame_def.fields.len();
                header.p_frame_def = p_frame_def;
            }
        } else if line.starts_with("H Field S name:") {
            // Parse S frame field names
            if let Some(field_str) = line.strip_prefix("H Field S name:") {
                let names: Vec<String> = field_str.split(',').map(|s| s.trim().to_string()).collect();
                header.s_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field G name:") {
            // Parse G frame field names
            if let Some(field_str) = line.strip_prefix("H Field G name:") {
                let names: Vec<String> = field_str.split(',').map(|s| s.trim().to_string()).collect();
                header.g_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field H name:") {
            // Parse H frame field names
            if let Some(field_str) = line.strip_prefix("H Field H name:") {
                let names: Vec<String> = field_str.split(',').map(|s| s.trim().to_string()).collect();
                header.h_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field I signed:") {
            parse_signed_info(line, &mut header.i_frame_def)?;
        } else if line.starts_with("H Field P signed:") {
            parse_signed_info(line, &mut header.p_frame_def)?;
        } else if line.starts_with("H Field S signed:") {
            parse_signed_info(line, &mut header.s_frame_def)?;
        } else if line.starts_with("H Field G signed:") {
            parse_signed_info(line, &mut header.g_frame_def)?;
        } else if line.starts_with("H Field H signed:") {
            parse_signed_info(line, &mut header.h_frame_def)?;
        } else if line.starts_with("H Field I predictor:") {
            parse_predictor_info(line, &mut header.i_frame_def)?;
        } else if line.starts_with("H Field P predictor:") {
            parse_predictor_info(line, &mut header.p_frame_def)?;
        } else if line.starts_with("H Field S predictor:") {
            parse_predictor_info(line, &mut header.s_frame_def)?;
        } else if line.starts_with("H Field G predictor:") {
            parse_predictor_info(line, &mut header.g_frame_def)?;
        } else if line.starts_with("H Field H predictor:") {
            parse_predictor_info(line, &mut header.h_frame_def)?;
        } else if line.starts_with("H Field I encoding:") {
            parse_encoding_info(line, &mut header.i_frame_def)?;
        } else if line.starts_with("H Field P encoding:") {
            parse_encoding_info(line, &mut header.p_frame_def)?;
        } else if line.starts_with("H Field S encoding:") {
            parse_encoding_info(line, &mut header.s_frame_def)?;
        } else if line.starts_with("H Field G encoding:") {
            parse_encoding_info(line, &mut header.g_frame_def)?;
        } else if line.starts_with("H Field H encoding:") {
            parse_encoding_info(line, &mut header.h_frame_def)?;
        } else {
            // Parse sysconfig values
            parse_sysconfig_line(line, &mut header.sysconfig);
        }
    }
    
    Ok(header)
}

fn parse_signed_info(line: &str, frame_def: &mut FrameDefinition) -> Result<()> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() < 2 {
        return Ok(());
    }
    
    let signed_str = parts[1].trim();
    let signed_values: Vec<bool> = signed_str.split(',')
        .map(|s| s.trim() == "1")
        .collect();
    
    frame_def.update_signed(&signed_values);
    Ok(())
}

fn parse_predictor_info(line: &str, frame_def: &mut FrameDefinition) -> Result<()> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() < 2 {
        return Ok(());
    }
    
    let predictor_str = parts[1].trim();
    let predictors: Result<Vec<u8>, _> = predictor_str.split(',')
        .map(|s| s.trim().parse())
        .collect();
    
    match predictors {
        Ok(predictors) => {
            frame_def.update_predictors(&predictors);
            Ok(())
        }
        Err(_) => Err(BBLError::InvalidHeader("Invalid predictor values".to_string()))
    }
}

fn parse_encoding_info(line: &str, frame_def: &mut FrameDefinition) -> Result<()> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() < 2 {
        return Ok(());
    }
    
    let encoding_str = parts[1].trim();
    let encodings: Result<Vec<u8>, _> = encoding_str.split(',')
        .map(|s| s.trim().parse())
        .collect();
    
    match encodings {
        Ok(encodings) => {
            frame_def.update_encoding(&encodings);
            Ok(())
        }
        Err(_) => Err(BBLError::InvalidHeader("Invalid encoding values".to_string()))
    }
}

fn parse_sysconfig_line(line: &str, sysconfig: &mut HashMap<String, i32>) {
    if let Some(config_str) = line.strip_prefix("H ") {
        let parts: Vec<&str> = config_str.splitn(2, ':').collect();
        if parts.len() == 2 {
            let key = parts[0].trim();
            let value_str = parts[1].trim();
            
            // Handle array values like motorOutput:48,2047
            if key == "motorOutput" && value_str.contains(',') {
                let values: Vec<&str> = value_str.split(',').collect();
                for (i, val) in values.iter().enumerate() {
                    if let Ok(int_val) = val.trim().parse::<i32>() {
                        sysconfig.insert(format!("{}[{}]", key, i), int_val);
                    }
                }
            } else if let Ok(value) = value_str.parse::<i32>() {
                sysconfig.insert(key.to_string(), value);
            }
        }
    }
}
