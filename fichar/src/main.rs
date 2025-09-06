use axum::{
    Json, Router,
    body::Body,
    extract::{Request, State, rejection::JsonRejection},
    http::{HeaderValue, Response, StatusCode},
    middleware::{self, Next},
    routing::post,
};
use axum_server::{Handle, tls_rustls::RustlsConfig};
use chrono::Datelike;
use clap::{Parser, Subcommand};
use fichar::{
    context::Context,
    gen_key,
    input::Input,
    key_to_hex,
    language::Language,
    output::{Output, OutputDaySpan, OutputMonth},
    state::AppState,
};
use indoc::{formatdoc, indoc};
use render::Renderer;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use telegram::Update;
use time_util::{DateTimeExt, TimeZoneExt};
use tokio::{
    signal,
    sync::mpsc::{self, Receiver, Sender},
};
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::{Level, info, warn};

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Subcommand)]
enum Command {
    Load {
        #[arg(long)]
        reset_hook: bool,
    },
    Init {
        #[arg(long)]
        domain: String,
        #[arg(long)]
        port: u16,
    },
}
impl Default for Command {
    fn default() -> Self {
        Self::Load { reset_hook: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TotalState {
    hook: Hook,
    app_state: AppState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Hook {
    port: u16,
    domain: String,
    bot_token: String,
    secret_token: String,
    cert_cert: String,
    cert_key: String,
}
impl Hook {
    fn reset(self) -> Self {
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
    fn init(bot_token: String, domain: String) -> Self {
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
    fn port(self, port: u16) -> Self {
        Self { port, ..self }
    }
    async fn set(&self) {
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

#[tokio::main]
async fn main() {
    let Args { command } = Args::parse();

    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    match command {
        Command::Load { reset_hook } => {
            let mut state = TotalState::load();

            if reset_hook {
                state.hook = state.hook.reset();
                state.hook.set().await;
            }

            let hook = state.hook.clone();

            let (i_sender, i_receiver) = mpsc::channel::<Input>(8);
            let (o_sender, o_receiver) = mpsc::channel::<(Output, Context)>(8);

            let processor = tokio::spawn(state.process_inputs(i_receiver, o_sender));
            let sender = tokio::spawn(sender(hook.bot_token.clone(), o_receiver));

            let app = Router::new()
                .route("/", post(handler))
                .with_state(i_sender)
                .layer(middleware::from_fn_with_state(
                    HeaderValue::from_str(&hook.secret_token).unwrap(),
                    check_secret_token,
                ))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                        .on_request(DefaultOnRequest::new().level(Level::INFO))
                        .on_response(DefaultOnResponse::new().level(Level::INFO)),
                );

            let tls_conf = RustlsConfig::from_pem(hook.cert_cert.into(), hook.cert_key.into())
                .await
                .unwrap();
            let handle = Handle::new();
            let server = axum_server::bind_rustls(([0, 0, 0, 0], hook.port).into(), tls_conf)
                .handle(handle.clone())
                .serve(app.into_make_service());

            termination_signal(handle);
            server.await.unwrap();

            let state = processor.await.unwrap();
            sender.await.unwrap();

            info!("graceful shutdown");
            state
        }
        Command::Init { domain, port } => {
            dotenvy::dotenv().ok();
            let bot_token = std::env::var("JUSTMESSAGE_TELEGRAM_BOT_TOKEN").unwrap();

            TotalState {
                app_state: AppState::new(),
                hook: Hook::init(bot_token, domain).port(port),
            }
        }
    }
    .save();
}

impl TotalState {
    const FILE_PATH: &str = "state.postcard";
    fn load() -> Self {
        let bytes = std::fs::read(Self::FILE_PATH).unwrap();
        postcard::from_bytes(&bytes).unwrap()
    }
    fn save(&self) {
        let bytes = postcard::to_allocvec(self).unwrap();
        std::fs::write(Self::FILE_PATH, &bytes).unwrap();
        info!("state writen to disk");
    }
    async fn process_inputs(
        mut self,
        mut receiver: Receiver<Input>,
        mut output: Sender<(Output, Context)>,
    ) -> Self {
        while let Some(input) = receiver.recv().await {
            self.app_state.input(input, &mut output).await;
        }
        self
    }
}

// async fn printer(payload: String) -> StatusCode {
//     println!("{payload}");
//     StatusCode::OK
// }

async fn handler(
    sender: State<Sender<Input>>,
    payload: Result<Json<Update>, JsonRejection>,
) -> StatusCode {
    match payload {
        Ok(Json(update)) => {
            println!("{update:#?}");
            if let Ok(input) = Input::try_from(update) {
                println!("{input:#?}");
                sender.send(input).await.unwrap();
            }
        }
        Err(rejection) => println!("{rejection:#?}"),
    }
    StatusCode::OK
}

async fn check_secret_token(
    State(secret_token): State<HeaderValue>,
    request: Request,
    next: Next,
) -> Result<Response<Body>, StatusCode> {
    info!("checking secret token");
    if request
        .headers()
        .get("X-Telegram-Bot-Api-Secret-Token")
        .map(|header| header == secret_token)
        .unwrap_or(false)
    {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

trait Logged {
    async fn logged(self);
}

impl<T, E: std::fmt::Debug, F: Future<Output = Result<T, E>>> Logged for F {
    async fn logged(self) {
        match self.await {
            Ok(_) => {}
            Err(err) => warn!("error: {err:?}"),
        }
    }
}

async fn sender(token: String, mut receiver: Receiver<(Output, Context)>) {
    let renderer = Renderer::new();
    while let Some((output, context)) = receiver.recv().await {
        match output {
            Output::Ok => {
                telegram::send_text(&token, "ok".into(), context.chat)
                    .logged()
                    .await;
            }
            Output::Failure => {
                telegram::send_text(&token, "fail".into(), context.chat)
                    .logged()
                    .await;
            }
            Output::PleasePromoteTheBot => {
                let text = match context.language {
                    Language::En => "Please, promote me to administrator in the group settings.",
                    Language::Es => {
                        "Por favor, promocioneme administrador en la configuración del grupo."
                    }
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .logged()
                    .await;
            }
            Output::YourAreNotPartOfAGroup => {
                let text = match context.language {
                    Language::En => "You are not part of a group.",
                    Language::Es => "No eres parte de une grupo.",
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .logged()
                    .await;
            }
            Output::CouldNotRecognizeCommand => {
                let text = match context.language {
                    Language::En => "The command you wrote is not recognized.",
                    Language::Es => "El comando que escribiste no está reconocido.",
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .logged()
                    .await;
            }
            Output::Help => {
                let text = match context.language {
                    Language::En => indoc! {"
                        Here are example of available commands:

                        month
                        18h30 21h00
                        enter
                        enter 18h30
                        leave
                        leave 21h00
                    "},
                    Language::Es => indoc! {"
                        Aqui son ejemplos de comandos disponibles:

                        mes
                        18h30 21h00
                        entra
                        entra 18h30
                        sale
                        sale 21h00
                    "},
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .logged()
                    .await;
            }
            Output::SpanHasEarlierLeaveThanEnter(span) => {
                let enter = context.time_zone.instant(span.enter);
                let leave = context.time_zone.instant(span.leave);
                let enter_ymd = enter.format_ymd("/");
                let leave_ymd = leave.format_ymd("/");
                let enter_hm = enter.format_hm("h");
                let leave_hm = leave.format_hm("h");

                let text = match context.language {
                    Language::En => formatdoc!(
                        "
                            The time span has leave instant earlier than enter instant:
                                - enter {enter_ymd} {enter_hm}
                                - leave {leave_ymd} {leave_hm}
                        ",
                    ),
                    Language::Es => formatdoc!(
                        "
                            El tramo de tiempo tiene instante de salida antes del instante de entrada:
                                - entra {enter_ymd} {enter_hm}
                                - sale {leave_ymd} {leave_hm}
                        ",
                    ),
                };
                telegram::send_text(&token, text, context.chat)
                    .logged()
                    .await;
            }
            Output::SpanOverrodeSpans(spans) => {
                use std::fmt::Write;
                let mut text = String::new();
                match (context.language, spans.len()) {
                    (Language::En, 2..) => "The following time spans were overriden:",
                    (Language::En, ..) => "The following time span was overriden:",
                    (Language::Es, 2..) => "Se anularon los siguientes tramos de tiempo:",
                    (Language::Es, ..) => "Se anuló el siguiente tramo de tiempo:",
                };
                let (from, to) = match context.language {
                    Language::En => ("from", "to"),
                    Language::Es => ("de", "a"),
                };
                for span in spans {
                    let enter = context.time_zone.instant(span.enter);
                    let leave = context.time_zone.instant(span.leave);
                    let date = enter.format_ymd("/");
                    let enter = enter.format_hm("h");
                    let leave = leave.format_hm("h");
                    writeln!(text, "  - {date} {from} {enter} {to} {leave}").unwrap();
                }
                telegram::send_text(&token, text, context.chat)
                    .logged()
                    .await;
            }
            Output::ClearedSpans(spans) => {
                use std::fmt::Write;
                let mut text = String::new();
                match (context.language, spans.len()) {
                    (Language::En, 2..) => "The following time spans were cleared:",
                    (Language::En, ..) => "The following time span was cleared:",
                    (Language::Es, 2..) => "Se anularon los siguientes tramos de tiempo:",
                    (Language::Es, ..) => "Se anuló el siguiente tramo de tiempo:",
                };
                let (from, to) = match context.language {
                    Language::En => ("from", "to"),
                    Language::Es => ("de", "a"),
                };
                for span in spans {
                    let enter = context.time_zone.instant(span.enter);
                    let leave = context.time_zone.instant(span.leave);
                    let date = enter.format_ymd("/");
                    let enter = enter.format_hm("h");
                    let leave = leave.format_hm("h");
                    writeln!(text, "  - {date} {from} {enter} {to} {leave}").unwrap();
                }
                telegram::send_text(&token, text, context.chat)
                    .logged()
                    .await;
            }
            Output::CouldNotInferMinute => {
                let text = match context.language {
                    Language::En => {
                        "I was not able to determine the time based on your indication."
                    }
                    Language::Es => {
                        "No era capaz de determinar el tiempo basandome en tu indicación."
                    }
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .logged()
                    .await;
            }
            Output::CouldNotInferDay => {
                let text = match context.language {
                    Language::En => {
                        "I was not able to determine the date based on your indication."
                    }
                    Language::Es => {
                        "No era capaz de determinar la fecha basandome en tu indicación."
                    }
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .logged()
                    .await;
            }
            Output::CouldNotInferMonth => {
                let text = match context.language {
                    Language::En => {
                        "I was not able to determine the month based on your indication."
                    }
                    Language::Es => "No era capaz de determinar el mes basandome en tu indicación.",
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .logged()
                    .await;
            }
            Output::EnterOverrodeEntered(enter) => {
                let enter = context.time_zone.instant(enter);

                let text = match context.language {
                    Language::En => "The previous entering time was overriden:",
                    Language::Es => "La hora de entrada previa se anuló:",
                };
                telegram::send_text(&token, format!("{text} {enter}"), context.chat)
                    .logged()
                    .await;
            }
            Output::TryLeaveButNotEntered => {
                let text = match context.language {
                    Language::En => {
                        "You are trying to leave, but you did not enter in the first place."
                    }
                    Language::Es => "Estás tratando de salir, pero no entraste en primer lugar.",
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .logged()
                    .await;
            }
            Output::Month {
                person: _,
                month,
                spans,
                name,
            } => {
                let month = context.time_zone.instant(month);

                let mut month = OutputMonth {
                    language: context.language,
                    name,
                    year: month.year(),
                    month: month.month(),
                    spans: Vec::new(),
                    minutes: 0,
                };
                for span in spans {
                    let enter = context.time_zone.instant(span.enter);
                    let leave = context.time_zone.instant(span.leave);
                    month.spans.push(OutputDaySpan {
                        date: enter.into(),
                        enter: enter.into(),
                        leave: leave.into(),
                        minutes: span.minutes(),
                    });
                    month.minutes += span.minutes();
                }

                let image = renderer.render(
                    include_str!("month.typ"),
                    HashMap::new(),
                    HashMap::from([(
                        "month.json",
                        serde_json::to_string_pretty(&month).unwrap().into_bytes(),
                    )]),
                );
                if let Ok(image) = image {
                    telegram::send_photo(&token, image, context.chat)
                        .logged()
                        .await;
                } else {
                    warn!("fail to generate document");
                }
            }
            Output::IAmNowAdministrator => {
                let text = match context.language {
                    Language::En => {
                        "I am now administrator in the group. I can now see messages published in the group and respond to them."
                    }
                    Language::Es => {
                        "Ahora soy administrador en el grupo. Ahora puedo ver los mensages publicados en el grupo y contestarlos."
                    }
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .logged()
                    .await;
            }
            Output::SpanAdded(span) => {
                let text = match context.language {
                    Language::En => "Time span registered:",
                    Language::Es => "Tramo de tiempo registrado:",
                };
                let text = format!("{}\n{}", text, span.format(&context));
                telegram::send_text(&token, text, context.chat)
                    .logged()
                    .await;
            }
            Output::Entered(enter) => {
                let enter = context.time_zone.instant(enter);
                let date = enter.format_ymd("/");
                let time = enter.format_hm("h");
                let text = match context.language {
                    Language::En => format!("You enter on {date} at {time}"),
                    Language::Es => format!("Entras el {date} a las {time}"),
                };
                telegram::send_text(&token, text, context.chat)
                    .logged()
                    .await;
            }
        }
    }
}

fn termination_signal(handle: Handle) {
    tokio::spawn(async move {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        handle.graceful_shutdown(None);
    });
}
