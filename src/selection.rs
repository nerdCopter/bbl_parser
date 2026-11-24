//! Log location and selection helpers for multi-log BBL files.
//!
//! A single BBL file can hold several flight logs back to back. These helpers
//! find where each log starts and validate a user-chosen log number using the
//! semantics of `blackbox_decode --index`: numbers are 1-based.

use std::fmt;

/// Header line that starts every log inside a BBL file.
pub const LOG_START_MARKER: &[u8] = b"H Product:Blackbox flight data recorder by Nicholas Sherlock";

/// Byte offsets of every log start marker in `data`, in file order.
pub fn find_log_offsets(data: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::new();
    if data.len() < LOG_START_MARKER.len() {
        return offsets;
    }
    for i in 0..=data.len() - LOG_START_MARKER.len() {
        if &data[i..i + LOG_START_MARKER.len()] == LOG_START_MARKER {
            offsets.push(i);
        }
    }
    offsets
}

/// Location of one log inside a multi-log file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogEntry {
    /// 1-based log number, as accepted by `--index`.
    pub number: usize,
    /// Start offset in bytes, relative to the first log start.
    pub offset: usize,
    /// Size in bytes up to the next log start (or the end of the data).
    pub size: usize,
}

/// Build the per-log offset/size listing from the offsets returned by
/// [`find_log_offsets`] and the total data length.
pub fn log_entries(offsets: &[usize], data_len: usize) -> Vec<LogEntry> {
    let first = offsets.first().copied().unwrap_or(0);
    offsets
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = offsets.get(i + 1).copied().unwrap_or(data_len);
            LogEntry {
                number: i + 1,
                offset: start - first,
                size: end - start,
            }
        })
        .collect()
}

/// Render the log listing printed when a log must be chosen with `--index`.
pub fn format_log_table(entries: &[LogEntry]) -> String {
    let mut out = String::from("Index  Start offset  Size (bytes)\n");
    for e in entries {
        out.push_str(&format!("{:5} {:13} {:13}\n", e.number, e.offset, e.size));
    }
    out
}

/// Error returned when a requested log number cannot be selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSelectError {
    /// Log numbers start at 1.
    Zero,
    /// The file holds fewer logs than requested.
    OutOfRange { requested: usize, total: usize },
}

impl fmt::Display for LogSelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogSelectError::Zero => write!(f, "Log numbers start at 1."),
            LogSelectError::OutOfRange { requested, total } => write!(
                f,
                "Couldn't load log #{requested} from this file, because there are only {total} logs in total."
            ),
        }
    }
}

impl std::error::Error for LogSelectError {}

/// Validate a 1-based log number against the number of logs in a file and
/// return the 0-based position of that log.
pub fn select_log(requested: usize, total: usize) -> Result<usize, LogSelectError> {
    if requested == 0 {
        Err(LogSelectError::Zero)
    } else if requested > total {
        Err(LogSelectError::OutOfRange { requested, total })
    } else {
        Ok(requested - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(b"junk");
        for body in [&b"AAAA"[..], &b"BBBBBB"[..], &b"C"[..]] {
            d.extend_from_slice(LOG_START_MARKER);
            d.extend_from_slice(body);
        }
        d
    }

    #[test]
    fn finds_every_marker_in_order() {
        let d = sample();
        let m = LOG_START_MARKER.len();
        assert_eq!(find_log_offsets(&d), vec![4, 4 + m + 4, 4 + 2 * m + 10]);
    }

    #[test]
    fn short_or_markerless_data_has_no_offsets() {
        assert!(find_log_offsets(b"").is_empty());
        assert!(find_log_offsets(b"H Product:Blackbox").is_empty());
        assert!(find_log_offsets(&LOG_START_MARKER[..LOG_START_MARKER.len() - 1]).is_empty());
    }

    #[test]
    fn marker_at_very_end_of_data_is_found() {
        assert_eq!(find_log_offsets(LOG_START_MARKER), vec![0]);
    }

    #[test]
    fn entries_are_relative_to_first_log_and_last_size_reaches_end() {
        let d = sample();
        let m = LOG_START_MARKER.len();
        let e = log_entries(&find_log_offsets(&d), d.len());
        assert_eq!(e.len(), 3);
        assert_eq!(
            e[0],
            LogEntry {
                number: 1,
                offset: 0,
                size: m + 4
            }
        );
        assert_eq!(
            e[1],
            LogEntry {
                number: 2,
                offset: m + 4,
                size: m + 6
            }
        );
        assert_eq!(
            e[2],
            LogEntry {
                number: 3,
                offset: 2 * m + 10,
                size: m + 1
            }
        );
    }

    #[test]
    fn table_matches_upstream_layout() {
        let t = format_log_table(&[LogEntry {
            number: 2,
            offset: 1234,
            size: 99,
        }]);
        assert_eq!(
            t,
            "Index  Start offset  Size (bytes)\n    2          1234            99\n"
        );
    }

    #[test]
    fn select_is_one_based() {
        assert_eq!(select_log(1, 4), Ok(0));
        assert_eq!(select_log(4, 4), Ok(3));
    }

    #[test]
    fn select_rejects_zero_and_out_of_range() {
        assert_eq!(select_log(0, 4), Err(LogSelectError::Zero));
        let e = select_log(5, 4).unwrap_err();
        assert_eq!(
            e.to_string(),
            "Couldn't load log #5 from this file, because there are only 4 logs in total."
        );
        assert!(select_log(usize::MAX, 4).is_err());
        assert!(select_log(1, 0).is_err());
    }
}
