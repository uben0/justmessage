use crate::State;
use chrono::TimeZone;
use chrono_tz::Tz;
use json::Json;
use just_message::{JustMessage, Message, Response};
use time_util::{Date, Time};

macro_rules! message {
    (
        $app:expr,
        $time:expr,
        $person:expr,
        $content:expr,
        $expect:expr $(,)?
    ) => {
        let result = $app.message(Message {
            instant: $time,
            person: $person,
            content: $content.to_string(),
        });
        for (result, expect) in result.into_iter().zip($expect.into_iter()) {
            expect(&result);
        }
    };
}

fn success(r: &Response) {
    assert_eq!(*r, Response::Success);
}

macro_rules! doc {
    ($($name:literal => $json:expr),* $(,)?) => {
        |doc: &Response| {
            let Response::Document { bytes, .. } = doc else { panic!("expecting document") };
            $(
                let Some(json) = bytes.get($name) else { panic!() };
                let json: Json = str::from_utf8(json).unwrap().parse().unwrap();
                if json != $json {
                    eprintln!("expecting {:#?}", $json);
                    eprintln!("but got {:#?}", json);
                    panic!();
                }
            )*
        }
    };
}

#[test]
fn test_01() {
    let mut app = State::default();
    app.time_zone = Tz::Europe__Madrid;

    let mut time = app
        .time_zone
        .with_ymd_and_hms(2025, 8, 12, 18, 52, 27)
        .unwrap()
        .timestamp();

    message!(app, time, 0, "enter", [success]);
    time += 135 * 60;
    message!(app, time, 0, "leave", [success]);
    let spans = Json::array([json_span(
        Date {
            year: 2025,
            month: 8,
            day: 12,
        },
        Time {
            hour: 18,
            minute: 52,
            second: 27,
        },
        Time {
            hour: 21,
            minute: 7,
            second: 27,
        },
        8100,
    )]);
    message!(app, time, 0, "2025/08", [doc!("spans.json" => spans)]);
}

fn json_span(date: Date, enters: Time, leaves: Time, seconds: u32) -> Json {
    Json::object([
        ("date", json_date(date)),
        ("enters", json_time(enters)),
        ("leaves", json_time(leaves)),
        ("seconds", seconds.into()),
    ])
}

fn json_date(date: Date) -> Json {
    Json::object([
        ("year", date.year.into()),
        ("month", date.month.into()),
        ("day", date.day.into()),
    ])
}
fn json_time(time: Time) -> Json {
    Json::object([
        ("hour", time.hour.into()),
        ("minute", time.minute.into()),
        ("second", time.second.into()),
    ])
}
