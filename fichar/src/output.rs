use std::fmt::Display;

use crate::{context::Context, language::Language, state::instance::Span};
use chrono::{DateTime, Datelike, TimeZone, Timelike};
use render::DocFormat;
use serde::Serialize;
use time_util::{DateTimeExt, TimeZoneExt};

#[derive(Debug, Clone)]
pub enum Output {
    PleasePromoteTheBot,
    Ok,
    Failure,
    YourAreNotPartOfAGroup,
    CouldNotRecognizeCommand,
    Help,
    SpanAdded(Span),
    Entered(i64),
    SpanHasEarlierLeaveThanEnter(Span),
    SpanOverrodeSpans(Vec<Span>),
    ClearedSpans {
        day: i64,
        spans: Vec<Span>,
    },
    EnterOverrodeEntered(i64),
    TryLeaveButNotEntered,
    CouldNotInferMinute,
    CouldNotInferDay,
    CouldNotInferMonth,
    Month {
        format: DocFormat,
        person: i64,
        name: String,
        month: i64,
        spans: Vec<Span>,
    },
    IAmNowAdministrator,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputMonth {
    pub language: Language,
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

pub struct SpanFormatter<'a> {
    context: &'a Context,
    span: Span,
}
impl<'a> Display for SpanFormatter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let enter = self.context.time_zone.instant(self.span.enter);
        let leave = self.context.time_zone.instant(self.span.leave);

        let from = match (self.context.language, enter.hour()) {
            (Language::En, ..) => "from",
            (Language::Es, 0..=1) => "de la",
            (Language::Es, 2..) => "de las",
        };
        let to = match (self.context.language, enter.hour()) {
            (Language::En, ..) => "to",
            (Language::Es, 0..=1) => "a la",
            (Language::Es, 2..) => "a las",
        };

        let date = enter.format_ymd("/");
        let enter = enter.format_hm("h");
        let leave = leave.format_hm("h");

        let minutes = self.span.minutes();
        let hours = minutes.div_euclid(60);
        let minutes = minutes.rem_euclid(60);

        writeln!(
            f,
            "▸ __{date}__ {from} {enter} {to} {leave} \\(_{hours}h{minutes:0>2}_\\)"
        )
    }
}
impl Span {
    pub fn format<'a>(self, context: &'a Context) -> SpanFormatter<'a> {
        SpanFormatter {
            context,
            span: self,
        }
    }
}
pub struct TimeFormatter<'a> {
    pub context: &'a Context,
    pub time: i64,
}
impl<'a> TimeFormatter<'a> {
    pub fn new(time: i64, context: &'a Context) -> Self {
        Self { time, context }
    }
}
impl<'a> Display for TimeFormatter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let time = self.context.time_zone.instant(self.time);
        let at = match (self.context.language, time.hour()) {
            (Language::En, ..) => "at",
            (Language::Es, 0..=1) => "a la",
            (Language::Es, 2..) => "a las",
        };
        let date = time.format_ymd("/");
        let time = time.format_hm("h");
        write!(f, "▸ __{date}__ {at} {time}")
    }
}
