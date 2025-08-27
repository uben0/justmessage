use crate::{
    command::{self, Command},
    context::Context,
    input::Input,
    language::Language,
    output::{Output, OutputMonth},
    state::instance::{AddSpanError, Instance, LeaveError, Span},
};
use chrono::{Datelike, TimeZone};
use chrono_tz::Tz;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::{Receiver, Sender, error::SendError};

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
    pub async fn command(
        &mut self,
        instance: i64,
        person: i64,
        date: i64,
        command: Command,
        output: &mut Vec<Output>,
    ) {
        let command = match command {
            Command::SpanHint { enter, leave } => match (
                enter.infer(self.instances[&instance].time_zone, date),
                leave.infer(self.instances[&instance].time_zone, date),
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
            Command::EnterHint { time_hint } => {
                match time_hint.infer(self.instances[&instance].time_zone, date) {
                    Some(enter) => Command::Enter { enter: enter.start },
                    None => {
                        output.push(Output::CouldNotInferMinute);
                        return;
                    }
                }
            }
            Command::LeaveHint { time_hint } => {
                match time_hint.infer(self.instances[&instance].time_zone, date) {
                    Some(leave) => Command::Leave { leave: leave.start },
                    None => {
                        output.push(Output::CouldNotInferMinute);
                        return;
                    }
                }
            }
            Command::MonthHint { time_hint } => {
                match time_hint.infer(self.instances[&instance].time_zone, date) {
                    Some(month) => Command::Month { month },
                    None => {
                        output.push(Output::CouldNotInferMonth);
                        return;
                    }
                }
            }
            other => other,
        };
        match command {
            Command::Help => {
                output.push(Output::Help);
            }
            Command::Nope => {}
            Command::Span { enter, leave } => {
                match self
                    .instances
                    .get_mut(&instance)
                    .unwrap()
                    .add_span(person, enter, leave)
                {
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
                }
            }
            Command::Enter { enter } => {
                match self
                    .instances
                    .get_mut(&instance)
                    .unwrap()
                    .enter(person, enter)
                {
                    Some(overriden) => {
                        output.push(Output::Ok);
                        output.push(Output::EnterOverrodeEntered(overriden));
                    }
                    None => {
                        output.push(Output::Ok);
                    }
                }
            }
            Command::Leave { leave } => {
                match self
                    .instances
                    .get_mut(&instance)
                    .unwrap()
                    .leave(person, leave)
                {
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
                }
            }
            Command::Month { month } => {
                let instance = self.instances.get(&instance).unwrap();
                let spans = instance.select(person, month.start, month.end);

                let month = instance
                    .time_zone
                    .timestamp_opt(month.start, 0)
                    .earliest()
                    .unwrap();
                output.push(Output::Ok);
                output.push(Output::Month(OutputMonth {
                    name: "Unknown".to_string(),
                    year: month.year(),
                    month: month.month(),
                    spans,
                }));
            }
            Command::SetTimeZone { time_zone } => {
                self.instances.get_mut(&instance).unwrap().time_zone = time_zone;
                output.push(Output::Ok);
            }
            Command::SetLanguage { language } => {
                self.instances.get_mut(&instance).unwrap().language = language;
                output.push(Output::Ok);
            }
            Command::SpanHint { enter, leave } => unreachable!(),
            Command::EnterHint { time_hint } => unreachable!(),
            Command::LeaveHint { time_hint } => unreachable!(),
            Command::MonthHint { time_hint } => unreachable!(),
        }
    }
    pub async fn input(&mut self, input: Input, output: &mut Sender<(Output, Context)>) {
        match input {
            Input::Text {
                chat,
                person,
                date,
                text,
            } => {
                let instance = if self.instances.contains_key(&chat) {
                    Some(chat)
                } else if let Some((&instance, _)) = self
                    .instances
                    .iter()
                    .find(|(_, instance)| instance.person(person).is_some())
                {
                    Some(instance)
                } else {
                    None
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
                            language: self.instances.get(&instance).unwrap().language,
                            time_zone: self.instances.get(&instance).unwrap().time_zone,
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
                                self.command(instance, person, date, command, &mut outputs)
                                    .await;
                                for this_output in outputs {
                                    output.send((this_output, context)).await.unwrap();
                                }
                            }
                        }
                    }
                }
            }
            Input::NewGroup { chat, name } => {
                self.instances
                    .insert(chat, Instance::new(name, Language::Es, Tz::Europe__Madrid));
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
