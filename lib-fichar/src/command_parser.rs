use crate::{Command, Error, PersonHint, Rule, TimeHintMinute, TimeHintMonth};
use chrono_tz::Tz;
use pest::{Parser, iterators::Pair};
use std::str::FromStr;

impl FromStr for Command {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Self::parse(Rule::command, s) {
            Ok(mut pairs) => {
                let command = pairs.next().unwrap().into_inner().next().unwrap();

                Ok(match command.as_rule() {
                    Rule::command_persons => Self::Persons,
                    Rule::command_enter => Self::EnterTimeHint(TimeHintMinute::None),
                    Rule::command_leave => Self::LeaveTimeHint(TimeHintMinute::None),
                    Rule::command_enter_hour_minute => {
                        let [hour, minute] = command.child().children();
                        Self::EnterTimeHint(TimeHintMinute::HourMinute(
                            parse_u32(hour),
                            parse_u32(minute),
                        ))
                    }
                    Rule::command_leave_hour_minute => {
                        let [hour, minute] = command.child().children();
                        Self::LeaveTimeHint(TimeHintMinute::HourMinute(
                            parse_u32(hour),
                            parse_u32(minute),
                        ))
                    }
                    Rule::command_month => Self::MonthHint {
                        person_hint: PersonHint::Me,
                        time_hint: TimeHintMonth::None,
                    },
                    Rule::command_month_month => {
                        let [month, targets] = command.children();
                        Self::MonthHint {
                            person_hint: PersonHint::Me,
                            time_hint: TimeHintMonth::Month(parse_month(month)),
                        }
                    }
                    Rule::command_month_year_month => {
                        let month = command.child();
                        let order = month.as_rule();
                        let [lhs, rhs] = month.children();
                        let (year, month) = match order {
                            Rule::year_month => (lhs, rhs),
                            Rule::month_year => (rhs, lhs),
                            _ => unreachable!(),
                        };
                        Self::MonthHint {
                            person_hint: PersonHint::Me,
                            time_hint: TimeHintMonth::YearMonth(
                                parse_year(year),
                                parse_month(month),
                            ),
                        }
                    }
                    Rule::command_set_time_zone => {
                        let time_zone = command.child();
                        Self::SetTimeZone {
                            time_zone: parse_time_zone(time_zone)?,
                        }
                    }
                    Rule::command_new_person => Self::PersonNew {
                        names: command
                            .into_inner()
                            .map(|name| name.as_str().to_string())
                            .collect(),
                        admin: false,
                    },
                    Rule::command_person_admin => {
                        let [person, admin] = command.children();
                        let person = parse_u32(person.child());
                        let admin = parse_bool(admin);
                        Self::PersonAdmin { person, admin }
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
fn parse_month(node: Pair<Rule>) -> u32 {
    match node.as_rule() {
        Rule::month_july => 7,
        Rule::month_august => 8,
        _ => todo!(),
    }
}
fn parse_bool(node: Pair<Rule>) -> bool {
    assert_eq!(node.as_rule(), Rule::bool);
    let node = node.into_inner().next().unwrap();
    match node.as_rule() {
        Rule::bool_true => true,
        Rule::bool_false => false,
        _ => panic!(),
    }
}
fn parse_u32(node: Pair<Rule>) -> u32 {
    assert_eq!(node.as_rule(), Rule::number);
    node.as_str().parse().unwrap()
}
fn parse_year(node: Pair<Rule>) -> i32 {
    assert_eq!(node.as_rule(), Rule::year);
    node.as_str().parse().unwrap()
}
fn parse_time_zone(node: Pair<Rule>) -> Result<Tz, Error> {
    assert_eq!(node.as_rule(), Rule::time_zone);
    match node.as_str() {
        "paris" | "Paris" => Ok(Tz::Europe__Paris),
        "madrid" | "Madrid" => Ok(Tz::Europe__Madrid),
        time_zone => time_zone
            .parse()
            .map_err(|_| Error::InvalidTimeZone(node.as_str().to_string())),
    }
}
trait NodeExt: Sized {
    fn child(self) -> Self;
    fn children<const N: usize>(self) -> [Self; N];
}
impl<'a> NodeExt for Pair<'a, Rule> {
    fn child(self) -> Self {
        self.into_inner().next().unwrap()
    }
    fn children<const N: usize>(self) -> [Self; N] {
        self.into_inner().fetch().unwrap()
    }
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
