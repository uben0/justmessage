use crate::language::Language;
use chrono_tz::Tz;
use std::ops::Range;
use time_util::{TimeHintDay, TimeHintMinute, TimeHintMonth};

mod parser;

pub use parser::parse;

pub enum Command {
    Help,
    Nope,
    Clear {
        day: Range<i64>,
    },
    ClearHint {
        day: TimeHintDay,
    },
    Span {
        enter: i64,
        leave: i64,
    },
    SpanHint {
        enter_day: Option<TimeHintDay>,
        enter_minute: TimeHintMinute,
        leave_day: Option<TimeHintDay>,
        leave_minute: TimeHintMinute,
    },
    Enter {
        enter: i64,
    },
    EnterHint {
        time_hint: TimeHintMinute,
    },
    Leave {
        leave: i64,
    },
    LeaveHint {
        time_hint: TimeHintMinute,
    },
    MonthHint {
        time_hint: TimeHintMonth,
    },
    Month {
        month: Range<i64>,
    },
    SetTimeZone {
        time_zone: Tz,
    },
    SetLanguage {
        language: Language,
    },
}
