use crate::state::instance::Span;
use chrono::{DateTime, Datelike, TimeZone, Timelike};
use serde::Serialize;

#[derive(Debug, Clone)]
pub enum Output {
    PleasePromoteTheBot,
    Ok,
    Failure,
    YourAreNotPartOfAGroup,
    CouldNotRecognizeCommand,
    Help,
    SpanHasEarlierLeaveThanEnter(Span),
    SpanOverrodeSpans(Vec<Span>),
    EnterOverrodeEntered(i64),
    TryLeaveButNotEntered,
    CouldNotInferMinute,
    CouldNotInferMonth,
    Month {
        person: i64,
        month: i64,
        spans: Vec<Span>,
    },
    IAmNowAdministrator,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputMonth {
    pub name: String,
    pub year: i32,
    pub month: u32,
    pub spans: Vec<OutputDaySpan>,
    pub minutes: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct OutputDaySpan {
    pub date: OutputDate,
    pub enter: OutputTime,
    pub leave: OutputTime,
    pub minutes: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct OutputDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct OutputTime {
    pub hour: u32,
    pub minute: u32,
}

impl<T: TimeZone> From<DateTime<T>> for OutputDate {
    fn from(date: DateTime<T>) -> Self {
        Self {
            year: date.year(),
            month: date.month(),
            day: date.day(),
        }
    }
}
impl<T: TimeZone> From<DateTime<T>> for OutputTime {
    fn from(time: DateTime<T>) -> Self {
        Self {
            hour: time.hour(),
            minute: time.minute(),
        }
    }
}
