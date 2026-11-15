//! Days are local days.
//!
//! A developer in Berlin and the same developer in Lisbon must not see "today"
//! move because a server counted in UTC. Everything is bucketed by the user's
//! calendar day, and the zone that produced the bucket is stored beside it, so a
//! later recount can disagree with the original without losing the original.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A calendar day in the user's zone, rendered as `YYYY-MM-DD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDay(NaiveDate);

impl LocalDay {
    pub fn new(date: NaiveDate) -> Self {
        Self(date)
    }

    /// The day an instant falls on, in `zone`.
    pub fn of(instant: DateTime<Utc>, zone: Tz) -> Self {
        Self(instant.with_timezone(&zone).date_naive())
    }

    pub fn date(self) -> NaiveDate {
        self.0
    }

    /// The first instant of this day in `zone`, as UTC.
    pub fn start_utc(self, zone: Tz) -> Option<DateTime<Utc>> {
        zone.from_local_datetime(&self.0.and_hms_opt(0, 0, 0)?)
            .single()
            .map(|local| local.with_timezone(&Utc))
    }

    /// Midnight local time exactly one day later, for range queries.
    pub fn end_utc(self, zone: Tz) -> Option<DateTime<Utc>> {
        self.start_utc(zone)
            .and_then(|start| start.checked_add_signed(Duration::days(1)))
    }

    pub fn nth_prior(self, days: u32) -> Self {
        Self(self.0 - Duration::days(i64::from(days)))
    }

    /// Every day from `first` to `last`, inclusive. Bounded: a caller asking for
    /// ten years gets a refusal, not a slow loop.
    pub fn range(first: Self, last: Self) -> Option<Vec<Self>> {
        if last < first {
            return None;
        }
        let span = last.0.signed_duration_since(first.0).num_days();
        if !(0..=3_660).contains(&span) {
            return None;
        }
        Some((0..=span).map(|offset| Self(first.0 + Duration::days(offset))).collect())
    }

    /// How the ledger keys a day.
    pub fn as_key(self) -> String {
        self.0.format("%Y-%m-%d").to_string()
    }

    pub fn parse(value: &str) -> Option<Self> {
        NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
            .ok()
            .map(Self)
    }
}

impl fmt::Display for LocalDay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.as_key())
    }
}

impl FromStr for LocalDay {
    type Err = DayParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        LocalDay::parse(value).ok_or(DayParseError)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("not a calendar day (expected YYYY-MM-DD)")]
pub struct DayParseError;

impl Serialize for LocalDay {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.as_key())
    }
}

impl<'de> Deserialize<'de> for LocalDay {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        LocalDay::parse(&value).ok_or_else(|| serde::de::Error::custom("not a calendar day"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;

    #[test]
    fn a_day_is_the_local_day_not_the_utc_one() {
        let late_evening = Utc.with_ymd_and_hms(2026, 3, 14, 23, 30, 0).unwrap();
        let berlin: Tz = "Europe/Berlin".parse().unwrap();
        assert_eq!(LocalDay::of(late_evening, berlin).as_key(), "2026-03-15");
        let utc: Tz = "UTC".parse().unwrap();
        assert_eq!(LocalDay::of(late_evening, utc).as_key(), "2026-03-14");
    }

    #[test]
    fn a_range_is_inclusive_and_bounded() {
        let first = LocalDay::parse("2026-11-01").unwrap();
        let last = LocalDay::parse("2026-11-03").unwrap();
        assert_eq!(LocalDay::range(first, last).unwrap().len(), 3);
        assert_eq!(LocalDay::range(first, first.nth_prior(1)), None);
        assert_eq!(
            LocalDay::range(LocalDay::parse("2000-01-01").unwrap(), last),
            None
        );
    }

    #[test]
    fn keys_round_trip() {
        let day = LocalDay::parse("2026-11-15").unwrap();
        assert_eq!(day.as_key(), "2026-11-15");
        assert_eq!(LocalDay::parse(&day.to_string()), Some(day));
        assert_eq!(LocalDay::parse("15/11/2026"), None);
    }
}
