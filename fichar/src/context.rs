use crate::language::Language;
use chrono_tz::Tz;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    pub chat: i64,
    pub date: i64,
    pub language: Language,
    pub time_zone: Tz,
}
