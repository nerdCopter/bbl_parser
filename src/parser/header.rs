use crate::types::{BBLHeader, FrameDefinition};
use anyhow::Result;
use std::collections::HashMap;

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
            header.firmware_revision = line
                .strip_prefix("H Firmware revision:")
                .unwrap_or("")
                .trim()
                .to_string();
        } else if line.starts_with("H Board information:") {
            header.board_info = line
                .strip_prefix("H Board information:")
                .unwrap_or("")
                .trim()
                .to_string();
        } else if line.starts_with("H Craft name:") {
            header.craft_name = line
                .strip_prefix("H Craft name:")
                .unwrap_or("")
                .trim()
                .to_string();
        } else if line.starts_with("H Data version:") {
            if let Ok(version) = line
                .strip_prefix("H Data version:")
                .unwrap_or("2")
                .trim()
                .parse()
            {
                header.data_version = version;
            }
        } else if line.starts_with("H Log start datetime:") {
            // Parse log start datetime for GPX timestamp generation
            // Format: "2024-10-10T18:37:25.559+00:00" or "0000-01-01T00:00:00.000+00:00" if not set
            if let Some(datetime_str) = line.strip_prefix("H Log start datetime:") {
                let datetime = datetime_str.trim().to_string();
                // Store even if it's the placeholder "0000-01-01..." so we know the header exists
                header.log_start_datetime = Some(datetime);
            }
        } else if line.starts_with("H looptime:") {
            if let Ok(lt) = line
                .strip_prefix("H looptime:")
                .unwrap_or("0")
                .trim()
                .parse()
            {
                header.looptime = lt;
            }
        } else if line.starts_with("H Field I name:") {
            // Parse I frame field names
            if let Some(field_str) = line.strip_prefix("H Field I name:") {
                let names: Vec<String> =
                    field_str.split(',').map(|s| s.trim().to_string()).collect();
                header.i_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field P name:") {
            // Parse P frame field names
            if let Some(field_str) = line.strip_prefix("H Field P name:") {
                let names: Vec<String> =
                    field_str.split(',').map(|s| s.trim().to_string()).collect();
                header.p_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field S name:") {
            // Parse S frame field names
            if let Some(field_str) = line.strip_prefix("H Field S name:") {
                let names: Vec<String> =
                    field_str.split(',').map(|s| s.trim().to_string()).collect();
                header.s_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field G name:") {
            // Parse G frame field names
            if let Some(field_str) = line.strip_prefix("H Field G name:") {
                let names: Vec<String> =
                    field_str.split(',').map(|s| s.trim().to_string()).collect();
                header.g_frame_def = FrameDefinition::from_field_names(names);
            }
        } else if line.starts_with("H Field H name:") {
            // Parse H frame field names
            if let Some(field_str) = line.strip_prefix("H Field H name:") {
                let names: Vec<String> =
                    field_str.split(',').map(|s| s.trim().to_string()).collect();
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
            // P frames inherit field names from I frames but have their own predictors
            if header.p_frame_def.field_names.is_empty()
                && !header.i_frame_def.field_names.is_empty()
            {
                header.p_frame_def =
                    FrameDefinition::from_field_names(header.i_frame_def.field_names.clone());
            }
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
            // P frames inherit field names from I frames but have their own encodings
            if header.p_frame_def.field_names.is_empty()
                && !header.i_frame_def.field_names.is_empty()
            {
                header.p_frame_def =
                    FrameDefinition::from_field_names(header.i_frame_def.field_names.clone());
            }
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
    let signed_values: Vec<bool> = signed_str.split(',').map(|s| s.trim() == "1").collect();

    frame_def.update_signed(&signed_values);
    Ok(())
}

fn parse_predictor_info(line: &str, frame_def: &mut FrameDefinition) -> Result<()> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() < 2 {
        return Ok(());
    }

    let predictor_str = parts[1].trim();
    let predictors: std::result::Result<Vec<u8>, _> =
        predictor_str.split(',').map(|s| s.trim().parse()).collect();

    match predictors {
        Ok(predictors) => {
            frame_def.update_predictors(&predictors);
            Ok(())
        }
        Err(_) => Err(anyhow::anyhow!("Invalid header: Invalid predictor values")),
    }
}

fn parse_encoding_info(line: &str, frame_def: &mut FrameDefinition) -> Result<()> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() < 2 {
        return Ok(());
    }

    let encoding_str = parts[1].trim();
    let encodings: std::result::Result<Vec<u8>, _> =
        encoding_str.split(',').map(|s| s.trim().parse()).collect();

    match encodings {
        Ok(encodings) => {
            frame_def.update_encoding(&encodings);
            Ok(())
        }
        Err(_) => Err(anyhow::anyhow!("Invalid header: Invalid encoding values")),
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
