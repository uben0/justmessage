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
use clap::{Parser, Subcommand, ValueEnum};
use fichar::{
    context::Context,
    input::Input,
    language::Language,
    output::{Output, OutputDaySpan, OutputMonth, TimeFormatter},
    state::AppState,
};
use indoc::{formatdoc, indoc};
use render::{DocFormat, Renderer};
use std::collections::HashMap;
use telegram::Update;
use time_util::{DateTimeExt, TimeZoneExt};
use tokio::{
    signal,
    sync::mpsc::{self, Receiver, Sender},
};
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::{Level, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
struct Args {
    env: Env,
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
        domain: String,
        port: u16,
    },
    SetToken,
    SetPort {
        port: u16,
    },
    SetDomain {
        domain: String,
    },
    Info,
}
impl Default for Command {
    fn default() -> Self {
        Self::Load { reset_hook: true }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Env {
    Prod,
    Dev,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let Args { env, command } = Args::parse();

    match env {
        Env::Prod => {
            tracing_subscriber::registry()
                .with(tracing_journald::layer().unwrap())
                .init();
        }
        Env::Dev => {
            tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
    }

    match command {
        Command::Info => {
            let state = AppState::load();
            println!("domain: {}", state.hook.domain);
            println!("  port: {}", state.hook.port);
        }
        Command::SetToken => {
            let mut state = AppState::load();
            state.hook.bot_token = get_token_from_env_var()?;
            state.save();
        }
        Command::SetPort { port } => {
            let mut state = AppState::load();
            state.hook.port = port;
            state.save();
        }
        Command::SetDomain { domain } => {
            let mut state = AppState::load();
            state.hook.domain = domain;
            state.save();
        }
        Command::Load { reset_hook } => {
            let mut state = AppState::load();

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
            state.save();
        }
        Command::Init { domain, port } => {
            match env {
                Env::Prod => {}
                Env::Dev => {
                    dotenvy::dotenv().ok();
                }
            }
            let bot_token = get_token_from_env_var()?;

            AppState::new(bot_token, domain, port).save();
        }
    }
    Ok(())
}

const TOKEN_ENV_VAR: &str = "JUSTMESSAGE_TELEGRAM_BOT_TOKEN";

#[derive(Debug)]
enum Error {
    TokenEnvVarNotFound,
}

fn get_token_from_env_var() -> Result<String, Error> {
    std::env::var(TOKEN_ENV_VAR).map_err(|_| Error::TokenEnvVarNotFound)
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
            // println!("{update:#?}");
            if let Ok(input) = Input::try_from(update) {
                // println!("{input:#?}");
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
                let line = match (context.language, spans.len()) {
                    (Language::En, 2..) => "The following time spans were overriden:",
                    (Language::En, ..) => "The following time span was overriden:",
                    (Language::Es, 2..) => "Se anularon los siguientes tramos de tiempo:",
                    (Language::Es, ..) => "Se anuló el siguiente tramo de tiempo:",
                };
                writeln!(text, "{line}").unwrap();
                for span in spans {
                    write!(text, "{}", span.format(&context)).unwrap();
                }
                telegram::send_markdown(&token, text, context.chat)
                    .logged()
                    .await;
            }
            Output::ClearedSpans { spans, day } if spans.is_empty() => {
                let day = context.time_zone.instant(day).format_ymd("/");
                let text = match context.language {
                    Language::En => {
                        format!("There are no registered time spans on the __{}__.", day)
                    }
                    Language::Es => format!("No hay tramo de tiempo registrado el __{}__.", day),
                };
                telegram::send_markdown(&token, text, context.chat)
                    .logged()
                    .await;
            }
            Output::ClearedSpans { spans, day: _ } => {
                use std::fmt::Write;
                let mut text = String::new();
                let line = match (context.language, spans.len()) {
                    (Language::En, 2..) => "The following time spans were cleared:",
                    (Language::En, ..) => "The following time span was cleared:",
                    (Language::Es, 2..) => "Se anularon los siguientes tramos de tiempo:",
                    (Language::Es, ..) => "Se anuló el siguiente tramo de tiempo:",
                };
                writeln!(text, "{line}").unwrap();
                for span in spans {
                    write!(text, "{}", span.format(&context)).unwrap();
                }
                telegram::send_markdown(&token, text, context.chat)
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
                let text = match context.language {
                    Language::En => "The previous entering time was overriden:",
                    Language::Es => "La hora de entrada previa se anuló:",
                };
                let enter = TimeFormatter::new(enter, &context);
                let text = format!("{text}\n{enter}");
                telegram::send_markdown(&token, text, context.chat)
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
                format,
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

                let document = renderer.render(
                    include_str!("month.typ"),
                    HashMap::new(),
                    HashMap::from([(
                        "month.json",
                        serde_json::to_string_pretty(&month).unwrap().into_bytes(),
                    )]),
                    format,
                );
                if let Ok(document) = document {
                    match format {
                        DocFormat::Png => {
                            telegram::send_photo(&token, document, context.chat)
                                .logged()
                                .await
                        }
                        DocFormat::Pdf => {
                            telegram::send_document(&token, document, context.chat)
                                .logged()
                                .await
                        }
                    }
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
                telegram::send_markdown(&token, text, context.chat)
                    .logged()
                    .await;
            }
            Output::Entered(enter) => {
                let text = match context.language {
                    Language::En => "You enter:",
                    Language::Es => "Entras:",
                };
                let enter = TimeFormatter::new(enter, &context);
                let text = format!("{text}\n{enter}");
                telegram::send_markdown(&token, text, context.chat)
                    .logged()
                    .await;
            }
        }
    }
}

/// Listens for termination signals and gracefully stops the web server
///
/// It will close all sending endpoint for input channel, which will
/// cause all sending endpoint for output channel to be closed. All tasks
/// will join and the service will gracefully exit.
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
