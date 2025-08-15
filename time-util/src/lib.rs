use chrono::{
    DateTime, Datelike, Days, Months, NaiveDate, NaiveTime, TimeDelta, TimeZone, Timelike,
};
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct Time {
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct DaySpan {
    date: Date,
    enters: Time,
    leaves: Time,
    seconds: u32,
}

pub trait TimeZoneExt: TimeZone + Clone {
    fn days(&self, range: Range<i64>) -> SpansDaySplit<Self> {
        SpansDaySplit(range, self.clone())
    }
}

pub trait DateTimeExt: Sized {
    fn align_year(self) -> Option<Self>;
    fn range_year(self) -> Option<Range<i64>>;
    fn align_month(self) -> Option<Self>;
    fn range_month(self) -> Option<Range<i64>>;
    fn align_day(self) -> Option<Self>;
    fn range_day(self) -> Option<Range<i64>>;
    fn align_hour(self) -> Option<Self>;
    fn range_hour(self) -> Option<Range<i64>>;
    fn align_minute(self) -> Option<Self>;
    fn range_minute(self) -> Option<Range<i64>>;
}
impl<T> DateTimeExt for DateTime<T>
where
    T: TimeZone,
{
    fn align_year(self) -> Option<Self> {
        self.with_nanosecond(0)?
            .with_second(0)?
            .with_minute(0)?
            .with_hour(0)?
            .with_day(1)?
            .with_month(1)
    }

    fn range_year(self) -> Option<Range<i64>> {
        assert_eq!(self.month(), 1);
        assert_eq!(self.day(), 1);
        assert_eq!(self.hour(), 0);
        assert_eq!(self.minute(), 0);
        assert_eq!(self.second(), 0);
        assert_eq!(self.nanosecond(), 0);
        let end = self.clone().checked_add_months(Months::new(12))?;
        Some(self.timestamp()..end.timestamp())
    }
    fn align_month(self) -> Option<Self> {
        self.with_nanosecond(0)?
            .with_second(0)?
            .with_minute(0)?
            .with_hour(0)?
            .with_day(1)
    }

    fn range_month(self) -> Option<Range<i64>> {
        assert_eq!(self.day(), 1);
        assert_eq!(self.hour(), 0);
        assert_eq!(self.minute(), 0);
        assert_eq!(self.second(), 0);
        assert_eq!(self.nanosecond(), 0);
        let end = self.clone().checked_add_months(Months::new(1))?;
        Some(self.timestamp()..end.timestamp())
    }
    fn align_day(self) -> Option<Self> {
        self.with_nanosecond(0)?
            .with_second(0)?
            .with_minute(0)?
            .with_hour(0)
    }

    fn range_day(self) -> Option<Range<i64>> {
        assert_eq!(self.hour(), 0);
        assert_eq!(self.minute(), 0);
        assert_eq!(self.second(), 0);
        assert_eq!(self.nanosecond(), 0);
        let end = self.clone().checked_add_days(Days::new(1))?;
        Some(self.timestamp()..end.timestamp())
    }
    fn align_hour(self) -> Option<Self> {
        self.with_nanosecond(0)?.with_second(0)?.with_minute(0)
    }

    fn range_hour(self) -> Option<Range<i64>> {
        assert_eq!(self.minute(), 0);
        assert_eq!(self.second(), 0);
        assert_eq!(self.nanosecond(), 0);
        let end = self.clone().checked_add_signed(TimeDelta::hours(1))?;
        Some(self.timestamp()..end.timestamp())
    }
    fn align_minute(self) -> Option<Self> {
        self.with_nanosecond(0)?.with_second(0)
    }

    fn range_minute(self) -> Option<Range<i64>> {
        assert_eq!(self.second(), 0);
        assert_eq!(self.nanosecond(), 0);
        let end = self.clone().checked_add_signed(TimeDelta::minutes(1))?;
        Some(self.timestamp()..end.timestamp())
    }
}

impl<T: TimeZone> TimeZoneExt for T {}

/// Time spans contained in the timestamp range
///
/// If the start and end timestamps belongs to the same day, a single span will be returned. Everytime a midnight is included in the range, a span will stop and a new will start.
pub struct SpansDaySplit<T: TimeZone>(pub Range<i64>, pub T);
impl<T: TimeZone> Iterator for SpansDaySplit<T> {
    type Item = DaySpan;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.start == self.0.end {
            return None;
        }
        assert!(self.0.start < self.0.end);
        let midnight = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        let start = self.1.timestamp_opt(self.0.start, 0).unwrap();
        let end = self.1.timestamp_opt(self.0.end, 0).unwrap();
        let prev_midnight = start.with_time(midnight).unwrap();
        let next_midnight = (prev_midnight + Days::new(1)).timestamp();

        if self.0.end <= next_midnight {
            let span = DaySpan {
                date: start.date_naive().into(),
                enters: start.time().into(),
                leaves: end.time().into(),
                seconds: (self.0.end - self.0.start) as u32,
            };
            self.0.start = self.0.end;
            Some(span)
        } else {
            let span = DaySpan {
                date: start.date_naive().into(),
                enters: start.time().into(),
                leaves: midnight.into(),
                seconds: (next_midnight - self.0.start) as u32,
            };
            self.0.start = next_midnight;
            Some(span)
        }
    }
}
impl From<NaiveDate> for Date {
    fn from(date: NaiveDate) -> Self {
        Self {
            year: date.year(),
            month: date.month(),
            day: date.day(),
        }
    }
}
impl From<NaiveTime> for Time {
    fn from(time: NaiveTime) -> Self {
        Self {
            hour: time.hour(),
            minute: time.minute(),
            second: time.second(),
        }
    }
}
