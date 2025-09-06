use crate::{
    command::{self, Command},
    context::Context,
    gen_key,
    input::Input,
    key_to_hex,
    language::Language,
    output::Output,
    state::instance::{AddSpanError, Instance, LeaveError, Span},
};
use axum::http::StatusCode;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{info, warn};

pub mod instance;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub port: u16,
    pub domain: String,
    pub bot_token: String,
    pub secret_token: String,
    pub cert_cert: String,
    pub cert_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub hook: Hook,
    instances: HashMap<i64, Instance>,
}
impl Hook {
    pub fn reset(self) -> Self {
        let certificate = rcgen::generate_simple_self_signed([self.domain.clone()]).unwrap();
        let cert_cert = certificate.cert.pem();
        let cert_key = certificate.signing_key.serialize_pem();
        let secret_token = key_to_hex(gen_key());

        Self {
            secret_token,
            cert_cert,
            cert_key,
            ..self
        }
    }
    pub fn init(bot_token: String, domain: String) -> Self {
        Self {
            port: 443,
            domain,
            bot_token,
            secret_token: String::new(),
            cert_cert: String::new(),
            cert_key: String::new(),
        }
        .reset()
    }
    pub fn port(self, port: u16) -> Self {
        Self { port, ..self }
    }
    pub async fn set(&self) {
        let mut cooldown = 8;
        while !telegram::set_webhook(
            &self.bot_token,
            format!("https://{}:{}", self.domain, self.port),
        )
        .drop_pending_updates()
        .certificate(self.cert_cert.clone().into())
        .secret_token(self.secret_token.clone())
        .send()
        .await
        .map(|response| response.status())
        .unwrap_or(StatusCode::BAD_REQUEST)
        .is_success()
        {
            warn!("failed to set webhook, retrying in {cooldown} seconds...");
            tokio::time::sleep(Duration::from_secs(cooldown)).await;
            cooldown *= 2;
        }
        info!("webhook set");
    }
}

impl AppState {
    const FILE_PATH: &str = "state.postcard";
    pub fn load() -> Self {
        let bytes = std::fs::read(Self::FILE_PATH).unwrap();
        postcard::from_bytes(&bytes).unwrap()
    }
    pub fn save(&self) {
        let bytes = postcard::to_allocvec(self).unwrap();
        std::fs::write(Self::FILE_PATH, &bytes).unwrap();
        info!("state writen to disk");
    }
    pub async fn process_inputs(
        mut self,
        mut receiver: Receiver<Input>,
        mut output: Sender<(Output, Context)>,
    ) -> Self {
        loop {
            tokio::select! {
                // auto-save, must be first to avoid starvation when lots of inputs arrive
                _ = tokio::time::sleep(Duration::from_secs(60 * 2)) => {
                    self.save();
                }
                input = receiver.recv() => {
                    let Some(input) = input else {
                        return self;
                    };
                    self.input(input, &mut output).await;
                }
            }
        }
    }
    pub fn new(bot_token: String, domain: String, port: u16) -> Self {
        Self {
            hook: Hook::init(bot_token, domain).port(port),
            instances: HashMap::new(),
        }
    }
    pub async fn input(&mut self, input: Input, output: &mut Sender<(Output, Context)>) {
        match input {
            Input::Text {
                user,
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
                        if let Some(first_name) = user.0 {
                            instance.set_first_name(person, first_name);
                        }
                        if let Some(last_name) = user.1 {
                            instance.set_last_name(person, last_name);
                        }
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
            Command::ClearHint { day } => match day.infer_past(self.time_zone, date) {
                Some(day) => Command::Clear { day },
                None => {
                    output.push(Output::CouldNotInferDay);
                    return;
                }
            },
            Command::SpanHint {
                enter_day: Some(enter_day),
                enter_minute,
                leave_day: Some(leave_day),
                leave_minute,
            } => {
                let Some(enter) = enter_day.infer_past(self.time_zone, date) else {
                    output.push(Output::CouldNotInferDay);
                    return;
                };
                let Some(leave) = leave_day.infer_first_after(self.time_zone, enter.start) else {
                    output.push(Output::CouldNotInferDay);
                    return;
                };
                match (
                    enter_minute.infer(self.time_zone, enter.start),
                    leave_minute.infer(self.time_zone, leave.start),
                ) {
                    (Some(enter), Some(leave)) => Command::Span {
                        enter: enter.start,
                        leave: leave.start,
                    },
                    (_, _) => {
                        output.push(Output::CouldNotInferMinute);
                        return;
                    }
                }
            }
            Command::SpanHint {
                enter_day: Some(enter_day),
                enter_minute,
                leave_day: None,
                leave_minute,
            } => {
                let Some(date) = enter_day.infer_past(self.time_zone, date) else {
                    output.push(Output::CouldNotInferDay);
                    return;
                };
                let Some(enter) = enter_minute.infer(self.time_zone, date.start) else {
                    output.push(Output::CouldNotInferMinute);
                    return;
                };
                let Some(leave) = leave_minute.infer_first_after(self.time_zone, enter.start)
                else {
                    output.push(Output::CouldNotInferMinute);
                    return;
                };
                Command::Span {
                    enter: enter.start,
                    leave: leave.start,
                }
            }
            Command::SpanHint {
                enter_day: None,
                enter_minute,
                leave_day: None,
                leave_minute,
            } => {
                let Some(enter) = enter_minute.infer(self.time_zone, date) else {
                    output.push(Output::CouldNotInferMinute);
                    return;
                };
                let Some(leave) = leave_minute.infer_first_after(self.time_zone, enter.start)
                else {
                    output.push(Output::CouldNotInferMinute);
                    return;
                };
                Command::Span {
                    enter: enter.start,
                    leave: leave.start,
                }
            }
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
                output.push(Output::Ok);
                output.push(Output::Help);
            }
            Command::Nope => {}
            Command::Clear { day } => {
                let removed = self.clear(person, day.start, day.end);
                output.push(Output::Ok);
                if !removed.is_empty() {
                    output.push(Output::ClearedSpans(removed));
                }
            }
            Command::Span { enter, leave } => match self.add_span(person, enter, leave) {
                Ok(overriden) if overriden.is_empty() => {
                    output.push(Output::Ok);
                    output.push(Output::SpanAdded(Span { enter, leave }));
                }
                Ok(overriden) => {
                    output.push(Output::Ok);
                    output.push(Output::SpanAdded(Span { enter, leave }));
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
                    output.push(Output::Entered(enter));
                    output.push(Output::EnterOverrodeEntered(overriden));
                }
                None => {
                    output.push(Output::Ok);
                    output.push(Output::Entered(enter));
                }
            },
            Command::Leave { leave } => match self.leave(person, leave) {
                Ok((added, overriden)) if overriden.is_empty() => {
                    output.push(Output::Ok);
                    output.push(Output::SpanAdded(added));
                }
                Ok((added, overriden)) => {
                    output.push(Output::Ok);
                    output.push(Output::SpanAdded(added));
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
                let name = self
                    .get_name(person)
                    .unwrap_or_else(|| "Unknown".to_string());
                output.push(Output::Ok);
                output.push(Output::Month {
                    person,
                    name,
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
            Command::ClearHint { .. } => unreachable!(),
            Command::SpanHint { .. } => unreachable!(),
            Command::EnterHint { .. } => unreachable!(),
            Command::LeaveHint { .. } => unreachable!(),
            Command::MonthHint { .. } => unreachable!(),
        }
    }
}
