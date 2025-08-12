use chrono::{DateTime, Datelike, TimeZone, Timelike};
use chrono_tz::Tz;
use just_message::{JustMessage, LocalDateTime, Message, Response};
use pest::Parser;
use pest_derive::Parser;
use serde::{Deserialize, Serialize};
pub use state::State;
use std::{collections::HashMap, str::FromStr};
use time_util::{Date, Time};

mod interpret;
mod state;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Person {
    name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub instant: i64,
    pub person: u32,
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
    InconsistentEntry(Entry),
    InvalidTimeZone(String),
    Parsing(String),
    InvalidDateTime(Date, Time),
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
}

impl Entry {
    pub fn enters(&self) -> bool {
        self.direction == Direction::Enters
    }
    pub fn leaves(&self) -> bool {
        self.direction == Direction::Leaves
    }
    pub fn key(&self) -> (i64, u32) {
        (self.instant, self.person)
    }
    // pub fn date_time(&self) -> (Date, Time) {
    //     let instant = DateTime::<Utc>::from_timestamp(self.instant, 0).unwrap();
    //     let date = Date {
    //         year: instant.year(),
    //         month: instant.month(),
    //         day: instant.day(),
    //     };
    //     let time = Time {
    //         hour: instant.hour(),
    //         minute: instant.minute(),
    //         second: instant.second(),
    //     };
    //     (date, time)
    // }
}

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub enum Command {
    Nope,
    Test,
    EnterNow,
    EnterNowHourMinute {
        hour: u32,
        minute: u32,
    },
    Enter {
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
    },
    LeaveNow,
    Leave {
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
    },
    PersonNew {
        name: String,
    },
    Month {
        year: i32,
        month: u32,
    },
    MonthNow,
    MonthNowMonth {
        month: u32,
    },
    SetTimeZone {
        time_zone: Tz,
    },
}

trait IterFetchArray: Iterator {
    fn fetch<const N: usize>(&mut self) -> Option<[Self::Item; N]> {
        let array = std::array::from_fn(|_| self.next());
        for elem in &array {
            if elem.is_none() {
                return None;
            }
        }
        Some(array.map(Option::unwrap))
    }
}
impl<T> IterFetchArray for T where T: Iterator {}

fn parse_month(rule: Rule) -> u32 {
    match rule {
        Rule::month_july => 7,
        Rule::month_august => 8,
        _ => todo!(),
    }
}

impl FromStr for Command {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Self::parse(Rule::command, s) {
            Ok(mut pairs) => {
                let command = pairs.next().unwrap().into_inner().next().unwrap();

                Ok(match command.as_rule() {
                    Rule::command_enter_now => Self::EnterNow,
                    Rule::command_enter_now_hour_minute => {
                        let [hour, minute] = command
                            .into_inner()
                            .next()
                            .unwrap()
                            .into_inner()
                            .fetch()
                            .unwrap();
                        Self::EnterNowHourMinute {
                            hour: hour.as_str().parse().unwrap(),
                            minute: minute.as_str().parse().unwrap(),
                        }
                    }
                    Rule::command_leave_now => Self::LeaveNow,
                    Rule::command_month_now => Self::MonthNow,
                    Rule::command_month_now_month => {
                        let month = command.into_inner().next().unwrap();
                        Self::MonthNowMonth {
                            month: parse_month(month.as_rule()),
                        }
                    }
                    Rule::command_month_year_month => {
                        let month = command.into_inner().next().unwrap();
                        let order = month.as_rule();
                        let [lhs, rhs] = month.into_inner().fetch().unwrap();
                        let (year, month) = match order {
                            Rule::year_month => (lhs, rhs),
                            Rule::month_year => (rhs, lhs),
                            _ => unreachable!(),
                        };
                        Self::Month {
                            year: year.as_str().parse().unwrap(),
                            month: parse_month(month.as_rule()),
                        }
                    }
                    Rule::command_set_time_zone => {
                        let time_zone = command.into_inner().next().unwrap();
                        Self::SetTimeZone {
                            time_zone: time_zone.as_str().parse().map_err(|_| {
                                Error::InvalidTimeZone(time_zone.as_str().to_string())
                            })?,
                        }
                    }
                    _ => {
                        dbg!(command);
                        todo!()
                    }
                })
            }
            Err(err) => Err(Error::Parsing(format!("{:?}", err))),
        }
    }
}

impl State {
    fn command(&mut self, command: Command, person: u32, instant: i64) -> Vec<Response> {
        match command {
            Command::SetTimeZone { time_zone } => {
                self.time_zone = time_zone;
                Vec::from([Response::Success])
            }
            Command::MonthNow => {
                let date_time = self.local_date_time(instant);
                self.command(
                    Command::Month {
                        year: date_time.year,
                        month: date_time.month,
                    },
                    person,
                    instant,
                )
            }
            Command::MonthNowMonth { month } => {
                let date_time = self.local_date_time(instant);
                self.command(
                    Command::Month {
                        year: date_time.year,
                        month,
                    },
                    person,
                    instant,
                )
            }
            Command::Nope => Vec::from([Response::Success]),
            Command::PersonNew { name } => {
                let person = self.new_person(name);
                [Response::Text(format!("new person {}", person))].into()
            }
            Command::EnterNowHourMinute { hour, minute } => {
                let date_time = self.local_date_time(instant);
                self.command(
                    Command::Enter {
                        year: date_time.year,
                        month: date_time.month,
                        day: date_time.day,
                        hour,
                        minute,
                    },
                    person,
                    instant,
                )
            }
            Command::EnterNow => match self.add_entry(person, Direction::Enters, instant) {
                Ok(()) => [Response::Success].into(),
                Err(err) => Response::err(&err),
            },
            Command::LeaveNow => match self.add_entry(person, Direction::Leaves, instant) {
                Ok(()) => [Response::Success].into(),
                Err(err) => Response::err(&err),
            },
            Command::Month { year, month } => match self.select_month(person, year, month) {
                Ok(spans) => [
                    Response::Success,
                    Response::Document {
                        main: include_str!("spans.typ").to_string(),
                        bytes: HashMap::from([(
                            "spans.json".to_string(),
                            serde_json::to_string_pretty(&spans).unwrap().into(),
                        )]),
                        sources: Default::default(),
                    },
                ]
                .into(),
                Err(err) => Response::err(&err),
            },
            Command::Test => [Response::Document {
                main: "HelloWorld".to_string(),
                bytes: Default::default(),
                sources: Default::default(),
            }]
            .into(),
            Command::Enter {
                year,
                month,
                day,
                hour,
                minute,
            } => {
                match self
                    .time_zone
                    .with_ymd_and_hms(year, month, day, hour, minute, 0)
                    .single()
                {
                    Some(instant) => {
                        match self.add_entry(person, Direction::Enters, instant.timestamp()) {
                            Ok(()) => [Response::Success].into(),
                            Err(err) => Response::err(&err),
                        }
                    }
                    None => Response::err(&Error::InvalidDateTime(
                        Date { year, month, day },
                        Time {
                            hour,
                            minute,
                            second: 0,
                        },
                    )),
                }
            }
            Command::Leave {
                year,
                month,
                day,
                hour,
                minute,
            } => {
                let instant = self
                    .time_zone
                    .with_ymd_and_hms(year, month, day, hour, minute, 0)
                    .unwrap()
                    .timestamp();
                match self.add_entry(person, Direction::Leaves, instant) {
                    Ok(()) => [Response::Success].into(),
                    Err(err) => Response::err(&err),
                }
            }
        }
    }
}

impl JustMessage for State {
    fn message(&mut self, message: Message) -> Vec<Response> {
        match message.content.parse() {
            Ok(command) => self.command(command, message.person, message.instant),
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
