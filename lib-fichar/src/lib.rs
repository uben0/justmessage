use chrono::{Datelike, Offset, TimeZone, Timelike};
use chrono_tz::Tz;
use indoc::indoc;
use just_message::{JustMessage, Message, Response};
use pest_derive::Parser;
use serde::{Deserialize, Serialize};
pub use state::State;
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};
use time_util::{Date, DaySpan, LocalDateTime, Time, TimeHintMinute, TimeHintMonth, TimeZoneExt};

mod command_parser;
mod state;
#[cfg(test)]
mod test;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Person {
    names: Vec<String>,
    admin: bool,
    entered: Option<i64>,
    spans: Vec<Span>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Span {
    enter: i64,
    leave: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Direction {
    Enters,
    Leaves,
}

#[derive(Debug)]
pub enum Error {
    InvalidSpan {
        enter: LocalDateTime,
        leave: LocalDateTime,
    },
    InvalidPerson(u32),
    InvalidMonth(u32),
    InconsistentEntry(Span),
    InvalidTimeZone(String),
    Parsing(pest::error::Error<Rule>),
    InvalidTimeHint,
    InvalidDateTime(Date, Time),
    InvalidTimeOp,
    PermissionDenied,
    ExpectingOnePerson,
    NotEnteredYet,
}

mod validate {
    use just_message::JustMessage;

    use super::{Error, State};
    pub fn person(person: u32, state: &State) -> Result<(), Error> {
        state.person(person)?;
        Ok(())
    }
    pub fn admin(person: u32, state: &State) -> Result<(), Error> {
        if state.person(person)?.admin {
            Ok(())
        } else {
            Err(Error::PermissionDenied)
        }
    }
    pub fn span(enter: i64, leave: i64, state: &State) -> Result<(), Error> {
        if enter < leave {
            Ok(())
        } else {
            Err(Error::InvalidSpan {
                enter: state.local_date_time(enter),
                leave: state.local_date_time(leave),
            })
        }
    }
}

// impl Span {
//     pub fn enters(&self) -> bool {
//         self.direction == Direction::Enters
//     }
//     pub fn leaves(&self) -> bool {
//         self.direction == Direction::Leaves
//     }
//     pub fn key(&self) -> (i64, u32) {
//         (self.instant, self.person)
//     }
// }

#[derive(Debug, Clone)]
enum PersonHint {
    Me,
    All,
    Index(u32),
    Name(String),
}
impl PersonHint {
    fn infer_one(self, me: u32) -> Result<u32, Error> {
        match self {
            Self::Me => Ok(me),
            Self::All => Err(Error::ExpectingOnePerson),
            Self::Index(person) => Ok(person),
            Self::Name(_) => todo!(),
        }
    }
    fn infer_any(self, me: u32, state: &State) -> HashSet<u32> {
        match self {
            PersonHint::Me => HashSet::from([me]),
            PersonHint::All => state.persons().collect(),
            PersonHint::Index(person) => HashSet::from([person]),
            PersonHint::Name(_) => todo!(),
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[grammar = "grammar.pest"]
#[grammar = "grammar-en.pest"]
pub enum Command {
    Help,
    Nope,
    Persons,
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
    PersonNew {
        names: Vec<String>,
        admin: bool,
    },
    MonthHint {
        person_hint: Vec<PersonHint>,
        time_hint: TimeHintMonth,
    },
    Month {
        persons: HashSet<u32>,
        month: Range<i64>,
    },
    SetTimeZone {
        time_zone: Tz,
    },
    PersonAdmin {
        person: u32,
        admin: bool,
    },
}

enum Output {
    None,
    Help,
    Month(Vec<OutputMonth>),
    NewPerson(u32),
    Persons(Vec<(u32, String)>),
    RemovedSpans(Vec<DaySpan>),
    Enter { previous: Option<LocalDateTime> },
}
#[derive(Debug, Clone, Serialize)]
struct OutputMonth {
    name: String,
    year: i32,
    month: u32,
    spans: Vec<DaySpan>,
}

impl State {
    fn command(&mut self, command: Command, person: u32, instant: i64) -> Result<Output, Error> {
        match command {
            Command::Enter { enter } => Ok(Output::Enter {
                previous: self
                    .enters(person, enter)?
                    .map(|instant| self.local_date_time(instant)),
            }),
            Command::Leave { leave } => self.leaves(person, leave).map(|spans| {
                Output::RemovedSpans(
                    spans
                        .into_iter()
                        .flat_map(|span| self.time_zone.days(span.enter..span.leave))
                        .collect(),
                )
            }),
            Command::EnterHint { time_hint } => self.command(
                Command::Enter {
                    enter: time_hint
                        .infer(self.time_zone, instant)
                        .ok_or(Error::InvalidTimeHint)?
                        .start,
                },
                person,
                instant,
            ),
            Command::LeaveHint { time_hint } => self.command(
                Command::Leave {
                    leave: time_hint
                        .infer(self.time_zone, instant)
                        .ok_or(Error::InvalidTimeHint)?
                        .start,
                },
                person,
                instant,
            ),
            Command::Help => Ok(Output::Help),
            Command::Persons => Ok(Output::Persons(
                self.persons()
                    .map(|i| (i, self.person(i).unwrap().names.join(" ")))
                    .collect(),
            )),
            Command::PersonAdmin { person, admin } => {
                validate::admin(person, self)?;
                self.set_person_admin(person, admin)?;
                Ok(Output::None)
            }
            Command::MonthHint {
                mut person_hint,
                time_hint,
            } => {
                if person_hint.is_empty() {
                    person_hint = Vec::from([PersonHint::Me]);
                }
                self.command(
                    Command::Month {
                        persons: person_hint
                            .into_iter()
                            .flat_map(|hint| hint.infer_any(person, self))
                            .collect(),
                        month: time_hint
                            .infer(self.time_zone, instant)
                            .ok_or(Error::InvalidTimeHint)?,
                    },
                    person,
                    instant,
                )
            }
            Command::SpanHint { enter, leave } => self.command(
                Command::Span {
                    enter: enter
                        .infer(self.time_zone, instant)
                        .ok_or(Error::InvalidTimeHint)?
                        .start,
                    leave: leave
                        .infer(self.time_zone, instant)
                        .ok_or(Error::InvalidTimeHint)?
                        .start,
                },
                person,
                instant,
            ),
            Command::Span { enter, leave } => self.add_span(person, enter, leave).map(|spans| {
                Output::RemovedSpans(
                    spans
                        .into_iter()
                        .flat_map(|span| self.time_zone.days(span.enter..span.leave))
                        .collect(),
                )
            }),
            Command::SetTimeZone { time_zone } => {
                validate::admin(person, self)?;
                self.time_zone = time_zone;
                Ok(Output::None)
            }
            Command::Nope => Ok(Output::None),
            Command::PersonNew { names, admin } => {
                let person = self.new_person(names, admin);
                Ok(Output::NewPerson(person))
            }
            Command::Month { persons, month } => {
                let date = self.local_date_time(month.start);
                Ok(Output::Month(
                    persons
                        .into_iter()
                        .map(|person| {
                            Ok(OutputMonth {
                                name: self.person(person)?.names.join(" "),
                                year: date.year,
                                month: date.month,
                                spans: self.select(person, month.clone())?,
                            })
                        })
                        .collect::<Result<Vec<OutputMonth>, Error>>()?,
                ))
            }
        }
    }
}

const TEMPLATE_MONTH: &str = include_str!("spans.typ");

fn success(iter: impl IntoIterator<Item = Response>) -> Vec<Response> {
    let mut res = Vec::from([Response::Success]);
    res.extend(iter);
    res
}
fn failure(iter: impl IntoIterator<Item = Response>) -> Vec<Response> {
    let mut res = Vec::from([Response::Failure]);
    res.extend(iter);
    res
}

impl JustMessage for State {
    fn message(&mut self, message: Message) -> Vec<Response> {
        let result = message
            .content
            .parse()
            .map(|command| self.command(command, message.person, message.instant));
        let result = match result {
            Ok(result) => result,
            Err(error) => Err(error),
        };
        match result {
            Ok(res) => match res {
                Output::Enter { previous } => match previous {
                    Some(previous) => success([Response::Text(format!(
                        "overriden {}-{:0>2}-{:0>2} {}:{:0>2}",
                        previous.year, previous.month, previous.day, previous.hour, previous.minute
                    ))]),
                    None => success([]),
                },
                Output::Help => success([Response::Text(
                    indoc! {"
                            month
                            month @all
                            2024/08
                            2024/08 @all
                            persons
                            set time zone madrid
                            18h30 21h00
                        "}
                    .to_string(),
                )]),
                Output::RemovedSpans(spans) => success(spans.into_iter().map(|span| {
                    Response::Text(format!(
                        "Removed {}-{:0>2}-{:0>2} {}:{:0>2} {}:{:0>2}",
                        span.date.year,
                        span.date.month,
                        span.date.day,
                        span.enters.hour,
                        span.enters.minute,
                        span.leaves.hour,
                        span.leaves.minute
                    ))
                })),
                Output::Persons(persons) => success(
                    persons
                        .into_iter()
                        .map(|(index, name)| Response::Text(format!("@{} {}", index, name))),
                ),
                Output::None => success([]),
                Output::Month(months) => {
                    success(months.into_iter().map(|month| Response::Document {
                        main: TEMPLATE_MONTH,
                        sources: HashMap::new(),
                        bytes: HashMap::from([(
                            "month.json",
                            serde_json::to_string_pretty(&month).unwrap().into(),
                        )]),
                    }))
                }
                Output::NewPerson(person) => {
                    success([Response::Text(format!("Person @{} created", person))]).into()
                }
            },
            Err(Error::InvalidSpan { enter, leave }) => failure([Response::Text(format!(
                indoc! {"
                    Span has leave instant earlier than enter instant:
                        - enter {} {}
                        - leave {} {}
                "},
                enter.date().display_ymd("-"),
                enter.time().display_hm("h"),
                leave.date().display_ymd("-"),
                leave.time().display_hm("h")
            ))]),
            Err(err) => Response::err(&err),
        }
    }

    fn local_date_time(&self, instant: i64) -> LocalDateTime {
        let date_time = self.time_zone.timestamp_opt(instant, 0).earliest().unwrap();
        LocalDateTime {
            year: date_time.year(),
            month: date_time.month(),
            day: date_time.day(),
            week_day: date_time.weekday() as u32,
            hour: date_time.hour(),
            minute: date_time.minute(),
            second: date_time.second(),
            offset: date_time.offset().fix().local_minus_utc(),
        }
    }
}

// struct Displayer<T>(T);
// impl Display for Displayer<LocalDateTime> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         todo!()
//     }
// }
