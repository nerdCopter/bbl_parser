//! Per-field minimum and maximum values of a log, as printed by
//! `blackbox_decode --limits`.

use crate::types::BBLLog;

/// Observed range of one main-frame (I/P) field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLimit {
    pub name: String,
    pub min: i64,
    pub max: i64,
}

impl FieldLimit {
    /// `max - min`.
    pub fn range(&self) -> i64 {
        self.max - self.min
    }
}

/// Compute min/max for every I-frame field over all I and P frames of `log`.
///
/// Fields follow the I-frame definition order. A log with no I/P frames
/// reports 0 for every limit, as `blackbox_decode` does.
pub fn compute_field_limits(log: &BBLLog) -> Vec<FieldLimit> {
    let mut limits: Vec<FieldLimit> = log
        .header
        .i_frame_def
        .field_names
        .iter()
        .map(|n| FieldLimit {
            name: n.trim().to_string(),
            min: 0,
            max: 0,
        })
        .collect();
    let mut seen = vec![false; limits.len()];

    for frame in &log.frames {
        if frame.frame_type != 'I' && frame.frame_type != 'P' {
            continue;
        }
        for (i, limit) in limits.iter_mut().enumerate() {
            if let Some(&v) = frame.data.get(&limit.name) {
                let v = v as i64;
                if seen[i] {
                    limit.min = limit.min.min(v);
                    limit.max = limit.max.max(v);
                } else {
                    limit.min = v;
                    limit.max = v;
                    seen[i] = true;
                }
            }
        }
    }
    limits
}

/// Render the limits table in the `blackbox_decode --limits` layout.
pub fn format_field_limits(limits: &[FieldLimit]) -> String {
    let mut out = String::from("\n\n    Field name          Min          Max        Range\n");
    out.push_str("-----------------------------------------------------\n");
    for l in limits {
        out.push_str(&format!(
            "{:>14} {:>12} {:>12} {:>12}\n",
            l.name,
            l.min,
            l.max,
            l.range()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DecodedFrame, FrameDefinition};

    fn frame(t: char, vals: &[(&str, i32)]) -> DecodedFrame {
        DecodedFrame {
            frame_type: t,
            timestamp_us: 0,
            loop_iteration: 0,
            data: vals.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    fn log_with(frames: Vec<DecodedFrame>) -> BBLLog {
        let mut log = BBLLog::new(1, 1);
        log.header.i_frame_def =
            FrameDefinition::from_field_names(vec!["loopIteration".into(), "gyroADC[0]".into()]);
        log.frames = frames;
        log
    }

    #[test]
    fn min_max_range_over_i_and_p_frames() {
        let log = log_with(vec![
            frame('I', &[("loopIteration", 0), ("gyroADC[0]", -5)]),
            frame('P', &[("loopIteration", 1), ("gyroADC[0]", 12)]),
            frame('P', &[("loopIteration", 2), ("gyroADC[0]", 3)]),
        ]);
        let l = compute_field_limits(&log);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].min, l[0].max, l[0].range()), (0, 2, 2));
        assert_eq!((l[1].min, l[1].max, l[1].range()), (-5, 12, 17));
    }

    #[test]
    fn all_negative_and_single_frame_do_not_clamp_to_zero() {
        let log = log_with(vec![frame(
            'I',
            &[("loopIteration", 7), ("gyroADC[0]", -9)],
        )]);
        let l = compute_field_limits(&log);
        assert_eq!((l[1].min, l[1].max), (-9, -9));
        assert_eq!((l[0].min, l[0].max), (7, 7));
    }

    #[test]
    fn extreme_values_do_not_overflow_range() {
        let log = log_with(vec![
            frame('I', &[("gyroADC[0]", i32::MIN)]),
            frame('P', &[("gyroADC[0]", i32::MAX)]),
        ]);
        let l = compute_field_limits(&log);
        assert_eq!(l[1].range(), i32::MAX as i64 - i32::MIN as i64);
    }

    #[test]
    fn non_main_frames_are_ignored_and_empty_log_is_zero() {
        let log = log_with(vec![frame('S', &[("gyroADC[0]", 999)])]);
        let l = compute_field_limits(&log);
        assert!(l.iter().all(|x| x.min == 0 && x.max == 0));
    }

    #[test]
    fn table_layout_matches_upstream() {
        let t = format_field_limits(&[FieldLimit {
            name: "axisP[0]".into(),
            min: -3,
            max: 10,
        }]);
        let expected = [
            "",
            "",
            "    Field name          Min          Max        Range",
            "-----------------------------------------------------",
            "      axisP[0]           -3           10           13",
            "",
        ]
        .join("\n");
        assert_eq!(t, expected);
    }
}
