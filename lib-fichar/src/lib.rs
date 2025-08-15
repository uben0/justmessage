use chrono::{DateTime, Datelike, TimeZone, Timelike};
use chrono_tz::Tz;
use just_message::{JustMessage, LocalDateTime, Message, Response};
use pest_derive::Parser;
use serde::{Deserialize, Serialize};
pub use state::State;
use std::{collections::HashMap, ops::Range};
use time_util::{Date, DateTimeExt, Time};

mod command_parser;
mod interpret;
mod state;
#[cfg(test)]
mod test;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Person {
    names: Vec<String>,
    admin: bool,
    spans: Vec<Span>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Span {
    pub person: u32,
    pub instant: i64,
    pub direction: Direction,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Direction {
    Enters,
    Leaves,
}

#[derive(Debug)]
pub enum Error {
    InvalidPerson(u32),
    InvalidMonth(u32),
    InconsistentEntry(Span),
    InvalidTimeZone(String),
    Parsing(String),
    InvalidTimeHint,
    InvalidDateTime(Date, Time),
    InvalidTimeOp,
    PermissionDenied,
    ExpectingOnePerson,
}

mod validate {
    use super::{Error, State};
    pub fn month(month: u32) -> Result<(), Error> {
        if let (1..=12) = month {
            Ok(())
        } else {
            Err(Error::InvalidMonth(month))
        }
    }
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
}

impl Span {
    pub fn enters(&self) -> bool {
        self.direction == Direction::Enters
    }
    pub fn leaves(&self) -> bool {
        self.direction == Direction::Leaves
    }
    pub fn key(&self) -> (i64, u32) {
        (self.instant, self.person)
    }
}

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
    fn infer_any(self, me: u32, state: &State) -> Vec<u32> {
        match self {
            PersonHint::Me => Vec::from([me]),
            PersonHint::All => state.persons().collect(),
            PersonHint::Index(person) => Vec::from([person]),
            PersonHint::Name(_) => todo!(),
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[grammar = "grammar.pest"]
pub enum Command {
    Nope,
    Persons,
    EnterInstant(i64),
    EnterTimeHint(TimeHintMinute),
    LeaveInstant(i64),
    LeaveTimeHint(TimeHintMinute),
    PersonNew {
        names: Vec<String>,
        admin: bool,
    },
    MonthHint {
        person_hint: PersonHint,
        time_hint: TimeHintMonth,
    },
    Month {
        persons: Vec<u32>,
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

#[derive(Debug, Clone, Copy)]
pub enum TimeHintMonth {
    None,
    Month(u32),
    YearMonth(i32, u32),
}
impl TimeHintMonth {
    fn infer(self, time_zone: impl TimeZone, instant: i64) -> Option<Range<i64>> {
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
#[derive(Debug, Clone, Copy)]
pub enum TimeHintMinute {
    None,
    Hour(u32),
    HourMinute(u32, u32),
}
impl TimeHintMinute {
    fn infer(self, time_zone: impl TimeZone, instant: i64) -> Option<Range<i64>> {
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

enum Output {
    None,
    Month { json: Vec<Vec<u8>> },
    NewPerson(u32),
    Persons(Vec<(u32, String)>),
}

impl State {
    fn command(&mut self, command: Command, person: u32, instant: i64) -> Result<Output, Error> {
        match command {
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
                person_hint,
                time_hint,
            } => self.command(
                Command::Month {
                    persons: person_hint.infer_any(person, self),
                    month: time_hint
                        .infer(self.time_zone, instant)
                        .ok_or(Error::InvalidTimeHint)?,
                },
                person,
                instant,
            ),
            Command::EnterTimeHint(time_hint) => self.command(
                Command::EnterInstant(
                    time_hint
                        .infer(self.time_zone, instant)
                        .ok_or(Error::InvalidTimeHint)?
                        .start,
                ),
                person,
                instant,
            ),
            Command::LeaveTimeHint(time_hint) => self.command(
                Command::LeaveInstant(
                    time_hint
                        .infer(self.time_zone, instant)
                        .ok_or(Error::InvalidTimeHint)?
                        .start,
                ),
                person,
                instant,
            ),
            Command::SetTimeZone { time_zone } => {
                validate::admin(person, self)?;
                self.time_zone = time_zone;
                Ok(Output::None)
            }
            Command::Nope => Ok(Output::None),
            Command::PersonNew { names, admin } => {
                let person = self.new_person(names, admin);
                Ok(Output::NewPerson(person))
                // Ok(Response::Text(format!("new person {}", person)))
            }
            Command::Month { persons, month } => Ok(Output::Month {
                json: persons
                    .into_iter()
                    .map(|person| {
                        self.select(person, month.clone())
                            .map(|spans| serde_json::to_string_pretty(&spans).unwrap().into())
                    })
                    .collect::<Result<Vec<Vec<u8>>, Error>>()?,
            }),
            Command::EnterInstant(instant) => self
                .add_entry(person, Direction::Enters, instant)
                .map(|_| Output::None),
            Command::LeaveInstant(instant) => self
                .add_entry(person, Direction::Leaves, instant)
                .map(|_| Output::None),
            // cmd => todo!("{:#?}", cmd),
        }
    }
}

const TEMPLATE_MONTH: &str = include_str!("spans.typ");

impl JustMessage for State {
    fn message(&mut self, message: Message) -> Vec<Response> {
        match message.content.parse() {
            Ok(command) => match self.command(command, message.person, message.instant) {
                Ok(res) => match res {
                    Output::Persons(persons) => {
                        let mut responses = Vec::from([Response::Success]);
                        for (index, name) in persons {
                            responses.push(Response::Text(format!("@{} {}", index, name)));
                        }
                        responses
                    }
                    Output::None => [Response::Success].into(),
                    Output::Month { json } => {
                        let mut responses = Vec::from([Response::Success]);
                        for json in json {
                            responses.push(Response::Document {
                                main: TEMPLATE_MONTH,
                                sources: HashMap::new(),
                                bytes: HashMap::from([("spans.json", json)]),
                            });
                        }
                        responses
                    }
                    Output::NewPerson(person) => [
                        Response::Success,
                        Response::Text(format!("Person @{} created", person)),
                    ]
                    .into(),
                },
                Err(err) => Response::err(&err),
            },
            Err(err) => Response::err(&err),
        }
    }

    fn local_date_time(&self, instant: i64) -> LocalDateTime {
        let date_time = DateTime::from_timestamp(instant, 0).unwrap();
        LocalDateTime {
            year: date_time.year(),
            month: date_time.month(),
            day: date_time.day(),
            week_day: date_time.weekday() as u32,
            hour: date_time.hour(),
            minute: date_time.minute(),
            second: date_time.second(),
            offset: 0,
        }
    }
}
