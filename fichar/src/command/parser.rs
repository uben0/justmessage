use chrono::Weekday;
use chrono_tz::Tz;
use pest::Parser;
use pest::RuleType;
use pest::iterators::Pair;
use time_util::TimeHintDay;
use time_util::TimeHintMinute;
use time_util::TimeHintMonth;
use tracing::error;
use unicode_normalization::UnicodeNormalization;

use crate::{command::Command, language::Language};

pub mod en {
    use pest_derive::Parser;

    #[derive(Parser)]
    #[grammar = "command/grammar.pest"]
    #[grammar = "command/grammar-en.pest"]
    pub struct CommandParser;
}
pub mod es {
    use pest_derive::Parser;

    #[derive(Parser)]
    #[grammar = "command/grammar.pest"]
    #[grammar = "command/grammar-es.pest"]
    pub struct CommandParser;
}

macro_rules! common_node_def {
    ([$($rule:ident),* $(,)?]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[allow(non_camel_case_types)]
        enum Node {
            $($rule,)*
        }

    };
}
macro_rules! common_node_impl {
    ($language:ident, [$($rule:ident),* $(,)?]) => {
        impl From<$language::Rule> for Node {
            fn from(rule: $language::Rule) -> Node {
                match rule {
                    $($language::Rule::$rule => Node::$rule,)*
                }
            }
        }
        impl From<Node> for $language::Rule{
            fn from(node: Node) -> $language::Rule {
                match node {
                    $(Node::$rule => $language::Rule::$rule,)*
                }
            }
        }
    };
}

macro_rules! common_node {
    ([$($language:ident),* $(,)?], $rules:tt) => {
        common_node_def!($rules);
        $(common_node_impl!($language, $rules);)*
    };
}

common_node!(
    [en, es],
    [
        EOI,
        WHITESPACE,
        TIME_ZONE,
        CLEAR,
        NEW,
        ADMIN,
        SET,
        HELP,
        PERSON,
        LANGUAGE,
        PERSONS,
        TARGET_ALL,
        TARGET_ME,
        TRUE,
        FALSE,
        ENTER,
        LEAVE,
        MONTH,
        MONTH_01,
        MONTH_02,
        MONTH_03,
        MONTH_04,
        MONTH_05,
        MONTH_06,
        MONTH_07,
        MONTH_08,
        MONTH_09,
        MONTH_10,
        MONTH_11,
        MONTH_12,
        WEEKDAY_0,
        WEEKDAY_1,
        WEEKDAY_2,
        WEEKDAY_3,
        WEEKDAY_4,
        WEEKDAY_5,
        WEEKDAY_6,
        word,
        hour_minute,
        number,
        year,
        year_month,
        month_year,
        time_zone,
        name,
        bool,
        targets,
        target,
        target_index,
        month,
        command,
        command_help,
        command_persons,
        command_person_admin,
        command_new_person,
        command_set_time_zone,
        command_set_language,
        command_clear,
        command_clear_date,
        command_span,
        command_span_date,
        command_span_date_date,
        command_enter,
        command_enter_hour_minute,
        command_leave,
        command_leave_hour_minute,
        command_month,
        command_month_month,
        command_month_year_month,
        weekday,
        day,
        date_sep,
        year_month_day,
        month_day,
        date_hint,
    ]
);

pub fn parse(language: Language, s: &str) -> Result<Command, ()> {
    match language {
        Language::En => parse_typed::<en::CommandParser, en::Rule>(s),
        Language::Es => parse_typed::<es::CommandParser, es::Rule>(s),
    }
}

fn parse_typed<P, R>(s: &str) -> Result<Command, ()>
where
    P: Parser<R>,
    R: RuleType + From<Node> + Into<Node>,
{
    match P::parse(R::from(Node::command), s) {
        Ok(mut pairs) => {
            let command = pairs.next().unwrap().into_inner().next().unwrap();

            Ok(match command.as_rule().into() {
                Node::command_help => Command::Help,
                Node::command_span => {
                    let [enter, leave] = command.children();
                    let [hour, minute] = enter.children();
                    let enter_minute =
                        TimeHintMinute::HourMinute(parse_u32(hour), parse_u32(minute));
                    let [hour, minute] = leave.children();
                    let leave_minute =
                        TimeHintMinute::HourMinute(parse_u32(hour), parse_u32(minute));
                    Command::SpanHint {
                        enter_day: None,
                        enter_minute,
                        leave_day: None,
                        leave_minute,
                    }
                }
                Node::command_clear => Command::ClearHint {
                    day: TimeHintDay::None,
                },
                Node::command_clear_date => {
                    let date = command.child();
                    let day = parse_date_hint(date);
                    Command::ClearHint { day }
                }
                Node::command_span_date => {
                    let [date, enter, leave] = command.children();
                    let [hour, minute] = enter.children().map(parse_u32);
                    let enter_minute = TimeHintMinute::HourMinute(hour, minute);
                    let [hour, minute] = leave.children().map(parse_u32);
                    let leave_minute = TimeHintMinute::HourMinute(hour, minute);

                    Command::SpanHint {
                        enter_day: Some(parse_date_hint(date)),
                        enter_minute,
                        leave_day: None,
                        leave_minute,
                    }
                }
                Node::command_span_date_date => {
                    let [date1, enter, date2, leave] = command.children();
                    let [hour, minute] = enter.children().map(parse_u32);
                    let enter_minute = TimeHintMinute::HourMinute(hour, minute);
                    let [hour, minute] = leave.children().map(parse_u32);
                    let leave_minute = TimeHintMinute::HourMinute(hour, minute);

                    Command::SpanHint {
                        enter_day: Some(parse_date_hint(date1)),
                        enter_minute,
                        leave_day: Some(parse_date_hint(date2)),
                        leave_minute,
                    }
                }
                Node::command_enter => Command::EnterHint {
                    time_hint: TimeHintMinute::None,
                },
                Node::command_leave => Command::LeaveHint {
                    time_hint: TimeHintMinute::None,
                },
                Node::command_enter_hour_minute => {
                    let [hour, minute] = command.child().children();
                    Command::EnterHint {
                        time_hint: TimeHintMinute::HourMinute(parse_u32(hour), parse_u32(minute)),
                    }
                }
                Node::command_leave_hour_minute => {
                    let [hour, minute] = command.child().children();
                    Command::LeaveHint {
                        time_hint: TimeHintMinute::HourMinute(parse_u32(hour), parse_u32(minute)),
                    }
                }
                Node::command_month => Command::MonthHint {
                    time_hint: TimeHintMonth::None,
                },
                Node::command_month_month => {
                    let month = command.child();
                    Command::MonthHint {
                        time_hint: TimeHintMonth::Month(parse_month(month)),
                    }
                }
                Node::command_month_year_month => {
                    let month = command.child();
                    let order = month.as_rule().into();
                    let [lhs, rhs] = month.children();
                    let (year, month) = match order {
                        Node::year_month => (lhs, rhs),
                        Node::month_year => (rhs, lhs),
                        _ => unreachable!(),
                    };
                    Command::MonthHint {
                        time_hint: TimeHintMonth::YearMonth(parse_year(year), parse_month(month)),
                    }
                }
                Node::command_set_time_zone => {
                    let time_zone = command.child();
                    Command::SetTimeZone {
                        time_zone: parse_time_zone(time_zone)?,
                    }
                }
                Node::command_set_language => {
                    let language = command.child();
                    Command::SetLanguage {
                        language: parse_language(language)?,
                    }
                }
                node => {
                    error!("unexpected node during parsing: {node:?}");
                    return Err(());
                }
            })
        }
        Err(_) => Err(()),
    }
}
fn parse_month<R>(node: Pair<R>) -> u32
where
    R: RuleType + Into<Node>,
{
    match node.as_rule().into() {
        Node::MONTH_01 => 1,
        Node::MONTH_02 => 2,
        Node::MONTH_03 => 3,
        Node::MONTH_04 => 4,
        Node::MONTH_05 => 5,
        Node::MONTH_06 => 6,
        Node::MONTH_07 => 7,
        Node::MONTH_08 => 8,
        Node::MONTH_09 => 9,
        Node::MONTH_10 => 10,
        Node::MONTH_11 => 11,
        Node::MONTH_12 => 12,
        _ => unreachable!(),
    }
}
fn parse_date_hint<R>(node: Pair<R>) -> TimeHintDay
where
    R: RuleType + Into<Node>,
{
    debug_assert_eq!(node.as_rule().into(), Node::date_hint);
    let hint = node.child();
    match hint.as_rule().into() {
        Node::weekday => {
            let weekday = hint.child();
            match weekday.as_rule().into() {
                Node::WEEKDAY_0 => TimeHintDay::Weekday(Weekday::Mon),
                Node::WEEKDAY_1 => TimeHintDay::Weekday(Weekday::Tue),
                Node::WEEKDAY_2 => TimeHintDay::Weekday(Weekday::Wed),
                Node::WEEKDAY_3 => TimeHintDay::Weekday(Weekday::Thu),
                Node::WEEKDAY_4 => TimeHintDay::Weekday(Weekday::Fri),
                Node::WEEKDAY_5 => TimeHintDay::Weekday(Weekday::Sat),
                Node::WEEKDAY_6 => TimeHintDay::Weekday(Weekday::Sun),
                _ => unreachable!(),
            }
        }
        Node::year_month_day => {
            let [year, month, day] = hint.children();
            let year = parse_year(year);
            let month = parse_month(month);
            let day = parse_day(day);
            TimeHintDay::YearMonthDay(year, month, day)
        }
        Node::month_day => {
            let [month, day] = hint.children();
            let month = parse_month(month);
            let day = parse_day(day);
            TimeHintDay::MonthDay(month, day)
        }
        Node::day => TimeHintDay::Day(parse_day(hint)),
        _ => unreachable!(),
    }
}
// fn parse_bool<R>(node: Pair<R>) -> bool
// where
//     R: RuleType + Into<Node>,
// {
//     assert_eq!(node.as_rule().into(), Node::bool);
//     let node = node.into_inner().next().unwrap();
//     match node.as_rule().into() {
//         Node::bool_true => true,
//         Node::bool_false => false,
//         _ => panic!(),
//     }
// }
fn parse_u32<R>(node: Pair<R>) -> u32
where
    R: RuleType + Into<Node>,
{
    debug_assert_eq!(node.as_rule().into(), Node::number);
    node.as_str().parse().unwrap()
}
fn parse_day<R>(node: Pair<R>) -> u32
where
    R: RuleType + Into<Node>,
{
    debug_assert_eq!(node.as_rule().into(), Node::day);
    node.as_str().parse().unwrap()
}
fn parse_year<R>(node: Pair<R>) -> i32
where
    R: RuleType + Into<Node>,
{
    debug_assert_eq!(node.as_rule().into(), Node::year);
    node.as_str().parse().unwrap()
}
fn parse_time_zone<R>(node: Pair<R>) -> Result<Tz, ()>
where
    R: RuleType + Into<Node>,
{
    debug_assert_eq!(node.as_rule().into(), Node::time_zone);
    match node.as_str() {
        "paris" | "Paris" => Ok(Tz::Europe__Paris),
        "madrid" | "Madrid" => Ok(Tz::Europe__Madrid),
        time_zone => time_zone.parse().map_err(|_| ()),
    }
}
fn parse_language<R>(node: Pair<R>) -> Result<Language, ()>
where
    R: RuleType + Into<Node>,
{
    debug_assert_eq!(node.as_rule().into(), Node::word);
    let language = node.as_str().normalize();
    match language.as_str() {
        "en" | "english" | "ingles" => Ok(Language::En),
        "es" | "spanish" | "espanol" => Ok(Language::Es),
        _ => Err(()),
    }
}
trait NodeExt: Sized {
    fn child(self) -> Self;
    fn children<const N: usize>(self) -> [Self; N];
}
impl<'a, R> NodeExt for Pair<'a, R>
where
    R: RuleType,
{
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

trait StringNormalization {
    fn normalize(&self) -> String;
}
impl StringNormalization for str {
    fn normalize(&self) -> String {
        self.nfd()
            .filter(|&c| char::is_alphabetic(c))
            .flat_map(|c| c.to_lowercase())
            .collect()
    }
}

#[test]
fn test_string_normalization() {
    assert_eq!("marché".normalize(), "marche");
    assert_eq!("ESPAÑOL".normalize(), "espanol");
}
