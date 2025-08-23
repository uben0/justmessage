use chrono::{
    DateTime, Datelike, Days, Months, NaiveDate, NaiveTime, TimeDelta, TimeZone, Timelike,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Display, ops::Range};

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
    pub date: Date,
    pub enters: Time,
    pub leaves: Time,
    pub seconds: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub week_day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub offset: i32,
}
#[derive(Debug, Clone, Copy)]
pub enum TimeHintMinute {
    None,
    Hour(u32),
    HourMinute(u32, u32),
}
#[derive(Debug, Clone, Copy)]
pub enum TimeHintDay {
    None,
    Weekday(u32),
    Day(u32),
    MonthDay(u32, u32),
    YearMonth(i32, u32, u32),
}
#[derive(Debug, Clone, Copy)]
pub enum TimeHintMonth {
    None,
    Month(u32),
    YearMonth(i32, u32),
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

impl TimeHintMinute {
    pub fn infer(self, time_zone: impl TimeZone, instant: i64) -> Option<Range<i64>> {
        let instant = time_zone.timestamp_opt(instant, 0).single()?;
        Some(match self {
            Self::None => instant.align_minute()?.range_minute()?,
            Self::Hour(hour) => instant.align_day()?.with_hour(hour)?.range_minute()?,
            Self::HourMinute(hour, minute) => instant
                .align_day()?
                .with_hour(hour)?
                .with_minute(minute)?
                .range_minute()?,
        })
    }
}
impl TimeHintMonth {
    pub fn infer(self, time_zone: impl TimeZone, instant: i64) -> Option<Range<i64>> {
        Some(match self {
            Self::None => time_zone
                .timestamp_opt(instant, 0)
                .single()?
                .align_month()?
                .range_month()?,
            Self::Month(month) => time_zone
                .timestamp_opt(instant, 0)
                .single()?
                .align_year()?
                .with_month(month)?
                .range_month()?,
            Self::YearMonth(year, month) => time_zone
                .with_ymd_and_hms(year, month, 1, 0, 0, 0)
                .single()?
                .range_month()?,
        })
    }
}

impl LocalDateTime {
    pub fn date(&self) -> Date {
        Date {
            year: self.year,
            month: self.month,
            day: self.day,
        }
    }
    pub fn time(&self) -> Time {
        Time {
            hour: self.hour,
            minute: self.minute,
            second: self.second,
        }
    }
}

pub struct TimeDisplayHourMinute {
    time: Time,
    sep: &'static str,
}
impl Display for TimeDisplayHourMinute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}{:0>2}", self.time.hour, self.sep, self.time.minute)
    }
}
impl Time {
    pub fn display_hm(self, sep: &'static str) -> TimeDisplayHourMinute {
        TimeDisplayHourMinute { time: self, sep }
    }
}
pub struct DateDisplayYearMonthDay {
    date: Date,
    sep: &'static str,
}
impl Display for DateDisplayYearMonthDay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}{:0>2}{}{:0>2}",
            self.date.year, self.sep, self.date.month, self.sep, self.date.day
        )
    }
}
impl Date {
    pub fn display_ymd(self, sep: &'static str) -> DateDisplayYearMonthDay {
        DateDisplayYearMonthDay { date: self, sep }
    }
}

#[test]
fn test_time_hint_month() {
    use chrono::Utc;
    let ymd_hms = |year, month, day, hour, minute, second| {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
            .single()
            .unwrap()
            .timestamp()
    };
    let instant = ymd_hms(2025, 8, 21, 20, 15, 0);
    let month_start = ymd_hms(2025, 8, 1, 0, 0, 0);
    let month_end = ymd_hms(2025, 9, 1, 0, 0, 0);
    assert_eq!(
        TimeHintMonth::None.infer(Utc, instant),
        Some(month_start..month_end)
    );
}
