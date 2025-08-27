use crate::language::Language;
use chrono_tz::Tz;
use std::{collections::HashSet, ops::Range};
use time_util::{TimeHintMinute, TimeHintMonth};

mod parser;

pub use parser::parse;

pub enum Command {
    Help,
    Nope,
    Span {
        enter: i64,
        leave: i64,
    },
    SpanHint {
        enter: TimeHintMinute,
        leave: TimeHintMinute,
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
