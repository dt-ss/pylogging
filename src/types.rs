use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// A single log record: a map of field name to value, both borrowed for the
/// duration of the logging call (e.g. `"message"`, `"level"`, `"timestamp"`).
pub type Record<'a> = HashMap<&'a str, &'a str>;

/// Convenience methods on [`Record`].
pub trait RecordExt {
    /// Returns the value for `key`, or `""` if the key is absent.
    fn get_or_default(&self, key: &str) -> &str;
}

impl RecordExt for Record<'_> {
    fn get_or_default(&self, key: &str) -> &str {
        self.get(key).map_or("", |v| v)
    }
}

/// Severity level of a log message, ordered from least to most severe.
///
/// The numeric values are stable and used for cheap comparisons and for
/// storing the level in an atomic (`as u8` / [`Level::from_u8`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Fine-grained detail for diagnosing problems.
    Debug = 0,
    /// Routine confirmation that things are working.
    Info = 1,
    /// Something unexpected, or a problem that may occur soon.
    Warning = 2,
    /// A failure that prevented some operation from completing.
    Error = 3,
    /// A severe failure; the program may be unable to continue.
    Critical = 4,
}

impl Level {
    /// The uppercase name of the level, e.g. `Level::Info` -> `"INFO"`.
    pub fn to_str(&self) -> &str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warning => "WARNING",
            Level::Error => "ERROR",
            Level::Critical => "CRITICAL",
        }
    }
    /// The level's stable numeric value.
    pub fn to_u8(self) -> u8 {
        self as u8
    }
    /// Inverse of [`Level::to_u8`]. Panics on a value outside `0..=4`.
    pub fn from_u8(u8: u8) -> Level {
        match u8 {
            0 => Level::Debug,
            1 => Level::Info,
            2 => Level::Warning,
            3 => Level::Error,
            4 => Level::Critical,
            _ => panic!("Invalid level"),
        }
    }
}

/// The error returned when a string cannot be parsed into a [`Level`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseLevelError {
    /// The unrecognized input that failed to parse.
    pub input: String,
}

impl fmt::Display for ParseLevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid level: {}", self.input)
    }
}

impl std::error::Error for ParseLevelError {}

impl FromStr for Level {
    type Err = ParseLevelError;

    /// Parses a level from its uppercase name (e.g. `"INFO"`), erroring on an
    /// unknown string. Enables `"INFO".parse::<Level>()`.
    fn from_str(s: &str) -> Result<Level, Self::Err> {
        match s {
            "DEBUG" => Ok(Level::Debug),
            "INFO" => Ok(Level::Info),
            "WARNING" => Ok(Level::Warning),
            "ERROR" => Ok(Level::Error),
            "CRITICAL" => Ok(Level::Critical),
            _ => Err(ParseLevelError {
                input: s.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn record_get_or_default() {
        let mut record: Record = HashMap::new();
        record.insert("a", "1");
        assert_eq!(record.get_or_default("a"), "1");
        assert_eq!(record.get_or_default("missing"), "");
    }

    #[test]
    fn level_ordering_is_by_severity() {
        assert!(Level::Debug < Level::Info);
        assert!(Level::Info < Level::Warning);
        assert!(Level::Warning < Level::Error);
        assert!(Level::Error < Level::Critical);
    }

    #[test]
    fn level_u8_round_trips() {
        for level in [
            Level::Debug,
            Level::Info,
            Level::Warning,
            Level::Error,
            Level::Critical,
        ] {
            assert_eq!(Level::from_u8(level.to_u8()), level);
        }
    }

    #[test]
    fn level_str_round_trips() {
        for level in [
            Level::Debug,
            Level::Info,
            Level::Warning,
            Level::Error,
            Level::Critical,
        ] {
            // Exercise the idiomatic `.parse()` path (via the FromStr impl).
            assert_eq!(level.to_str().parse::<Level>().unwrap(), level);
        }
    }

    #[test]
    fn level_from_str_rejects_unknown() {
        assert!("NOPE".parse::<Level>().is_err());
    }

    #[test]
    #[should_panic]
    fn level_from_u8_panics_on_out_of_range() {
        Level::from_u8(42);
    }
}
