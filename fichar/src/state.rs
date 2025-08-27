use crate::{
    command::{self, Command},
    context::Context,
    input::Input,
    language::Language,
    output::{Output, OutputMonth},
    state::instance::{AddSpanError, Instance, LeaveError},
};
use chrono::{Datelike, TimeZone};
use chrono_tz::Tz;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
use time_util::TimeZoneExt;
use tokio::sync::mpsc::{Receiver, Sender};

pub mod instance;

pub struct AppState {
    instances: HashMap<i64, Instance>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
        }
    }
    pub async fn input(&mut self, input: Input, output: &mut Sender<(Output, Context)>) {
        match input {
            Input::Text {
                chat,
                group,
                person,
                date,
                text,
            } => {
                let instance = if group {
                    Some(
                        self.instances
                            .entry(chat)
                            .or_insert_with(Instance::new_spain)
                            .with_person(person),
                    )
                } else {
                    self.instances
                        .values_mut()
                        .find(|instance| instance.person(person).is_some())
                };

                match instance {
                    None => {
                        let context = Context {
                            chat,
                            date,
                            language: Language::En,
                            time_zone: Tz::UTC,
                        };
                        output
                            .send((Output::YourAreNotPartOfAGroup, context))
                            .await
                            .unwrap();
                    }
                    Some(instance) => {
                        let context = Context {
                            chat,
                            date,
                            language: instance.language,
                            time_zone: instance.time_zone,
                        };
                        match command::parse(context.language, &text) {
                            Err(()) => {
                                output
                                    .send((Output::CouldNotRecognizeCommand, context))
                                    .await
                                    .unwrap();
                            }
                            Ok(command) => {
                                let mut outputs = Vec::new();
                                instance.command(person, date, command, &mut outputs).await;
                                for this_output in outputs {
                                    output.send((this_output, context)).await.unwrap();
                                }
                            }
                        }
                    }
                }
            }
            Input::NewGroup { chat, name: _ } => {
                self.instances.insert(chat, Instance::new_spain());
                let context = Context {
                    chat,
                    date: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                    language: Language::En,
                    time_zone: Tz::UTC,
                };
                output
                    .send((Output::PleasePromoteTheBot, context))
                    .await
                    .unwrap();
            }
            Input::LeftChat { chat, person } => {
                if let Some(instance) = self.instances.get_mut(&chat) {
                    instance.remove_person(person);
                }
            }
            Input::NowAdmin { chat } => {
                let context = Context {
                    chat,
                    date: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                    language: Language::En,
                    time_zone: Tz::UTC,
                };
                output
                    .send((Output::IAmNowAdministrator, context))
                    .await
                    .unwrap();
            }
        }
    }

    pub async fn process_inputs(
        mut self,
        mut receiver: Receiver<Input>,
        mut output: Sender<(Output, Context)>,
    ) {
        while let Some(input) = receiver.recv().await {
            self.input(input, &mut output).await;
        }
    }
}

impl Instance {
    pub async fn command(
        &mut self,
        person: i64,
        date: i64,
        command: Command,
        output: &mut Vec<Output>,
    ) {
        let command = match command {
            Command::SpanHint { enter, leave } => match (
                enter.infer(self.time_zone, date),
                leave.infer(self.time_zone, date),
            ) {
                (Some(enter), Some(leave)) => Command::Span {
                    enter: enter.start,
                    leave: leave.start,
                },
                (_, _) => {
                    output.push(Output::CouldNotInferMinute);
                    return;
                }
            },
            Command::EnterHint { time_hint } => match time_hint.infer(self.time_zone, date) {
                Some(enter) => Command::Enter { enter: enter.start },
                None => {
                    output.push(Output::CouldNotInferMinute);
                    return;
                }
            },
            Command::LeaveHint { time_hint } => match time_hint.infer(self.time_zone, date) {
                Some(leave) => Command::Leave { leave: leave.start },
                None => {
                    output.push(Output::CouldNotInferMinute);
                    return;
                }
            },
            Command::MonthHint { time_hint } => match time_hint.infer(self.time_zone, date) {
                Some(month) => Command::Month { month },
                None => {
                    output.push(Output::CouldNotInferMonth);
                    return;
                }
            },
            other => other,
        };
        match command {
            Command::Help => {
                output.push(Output::Help);
            }
            Command::Nope => {}
            Command::Span { enter, leave } => match self.add_span(person, enter, leave) {
                Ok(overriden) if overriden.is_empty() => {
                    output.push(Output::Ok);
                }
                Ok(overriden) => {
                    output.push(Output::Ok);
                    output.push(Output::SpanOverrodeSpans(overriden));
                }
                Err(AddSpanError::LeaveEarlierThanEnter(span)) => {
                    output.push(Output::Failure);
                    output.push(Output::SpanHasEarlierLeaveThanEnter(span));
                }
            },
            Command::Enter { enter } => match self.enter(person, enter) {
                Some(overriden) => {
                    output.push(Output::Ok);
                    output.push(Output::EnterOverrodeEntered(overriden));
                }
                None => {
                    output.push(Output::Ok);
                }
            },
            Command::Leave { leave } => match self.leave(person, leave) {
                Ok(overriden) if overriden.is_empty() => {
                    output.push(Output::Ok);
                }
                Ok(overriden) => {
                    output.push(Output::Ok);
                    output.push(Output::SpanOverrodeSpans(overriden));
                }
                Err(LeaveError::NotEntered) => {
                    output.push(Output::Failure);
                    output.push(Output::TryLeaveButNotEntered);
                }
                Err(LeaveError::LeaveEarlierThanEnter(span)) => {
                    output.push(Output::Failure);
                    output.push(Output::SpanHasEarlierLeaveThanEnter(span));
                }
            },
            Command::Month { month } => {
                output.push(Output::Ok);
                output.push(Output::Month {
                    person,
                    month: month.start,
                    spans: self.select(person, month.start, month.end),
                });
            }
            Command::SetTimeZone { time_zone } => {
                self.time_zone = time_zone;
                output.push(Output::Ok);
            }
            Command::SetLanguage { language } => {
                self.language = language;
                output.push(Output::Ok);
            }
            Command::SpanHint { .. } => unreachable!(),
            Command::EnterHint { .. } => unreachable!(),
            Command::LeaveHint { .. } => unreachable!(),
            Command::MonthHint { .. } => unreachable!(),
        }
    }
}
