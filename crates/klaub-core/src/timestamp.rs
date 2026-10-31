//! A UTC timestamp that serialises as RFC 3339.
//!
//! The domain never reads the system clock; the [`Clock`](crate::ports::Clock)
//! port supplies the current time, which keeps tests deterministic.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer};
use serde::{Serialize, Serializer};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::error::{Error, Result};

/// A point in time, stored in UTC with second precision on the wire.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(OffsetDateTime);

impl Timestamp {
    /// Builds a timestamp from seconds since the Unix epoch.
    pub fn from_unix(seconds: i64) -> Result<Self> {
        OffsetDateTime::from_unix_timestamp(seconds)
            .map(Self)
            .map_err(|e| Error::invalid("timestamp", e))
    }

    /// Wraps an `OffsetDateTime`, normalised to UTC.
    pub fn from_datetime(value: OffsetDateTime) -> Self {
        Self(value.to_offset(time::UtcOffset::UTC))
    }

    /// Seconds since the Unix epoch.
    pub fn unix(self) -> i64 {
        self.0.unix_timestamp()
    }

    /// Parses an RFC 3339 string.
    pub fn parse(value: &str) -> Result<Self> {
        OffsetDateTime::parse(value, &Rfc3339)
            .map(Self::from_datetime)
            .map_err(|e| Error::invalid("timestamp", e))
    }

    /// Formats as RFC 3339 with whole seconds, e.g. `2026-10-31T12:00:00Z`.
    pub fn to_rfc3339(self) -> String {
        let truncated = self.0.replace_nanosecond(0).unwrap_or(self.0);
        truncated
            .format(&Rfc3339)
            .unwrap_or_else(|_| self.unix().to_string())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_rfc3339())
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Timestamp({self})")
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_rfc3339())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_rfc3339() {
        let ts = Timestamp::from_unix(1_793_448_000).unwrap();
        assert_eq!(ts.to_rfc3339(), "2026-10-31T12:00:00Z");
        assert_eq!(Timestamp::parse("2026-10-31T12:00:00Z").unwrap(), ts);
        assert_eq!(Timestamp::parse("2026-10-31T09:00:00-03:00").unwrap(), ts);
    }

    #[test]
    fn serde_uses_strings() {
        let ts = Timestamp::from_unix(0).unwrap();
        let json = serde_json::to_string(&ts).unwrap();
        assert_eq!(json, "\"1970-01-01T00:00:00Z\"");
        assert_eq!(serde_json::from_str::<Timestamp>(&json).unwrap(), ts);
        assert!(serde_json::from_str::<Timestamp>("\"yesterday\"").is_err());
    }
}
