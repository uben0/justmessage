use crate::state::instance::Span;
use serde::Serialize;
use time_util::DaySpan;

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
    Month(OutputMonth),
    IAmNowAdministrator,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutputMonth {
    pub name: String,
    pub year: i32,
    pub month: u32,
    pub spans: Vec<DaySpan>,
}
