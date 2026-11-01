//! Clock and identifier adapters backed by the operating system.

use crate::domain::ports::{Clock, IdGenerator};
use crate::domain::timestamp::Timestamp;
use time::OffsetDateTime;

/// The system clock.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_datetime(OffsetDateTime::now_utc())
    }
}

/// Random identifiers from UUID v4.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RandomIds;

impl IdGenerator for RandomIds {
    fn random_hex(&self) -> String {
        uuid::Uuid::new_v4().simple().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_long_hex_and_distinct() {
        let ids = RandomIds;
        let (a, b) = (ids.random_hex(), ids.random_hex());
        assert!(a.len() >= 12 && a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn the_clock_is_after_2025() {
        assert!(SystemClock.now().unix() > 1_735_689_600);
    }
}
