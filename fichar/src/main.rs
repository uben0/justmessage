use axum::{
    Json, Router,
    body::Body,
    extract::{Request, State, rejection::JsonRejection},
    http::{HeaderValue, Response, StatusCode},
    middleware::{self, Next},
    routing::post,
};
use axum_server::tls_rustls::RustlsConfig;
use chrono::{Datelike, TimeZone};
use clap::Parser;
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
use std::collections::HashMap;
use telegram::Update;
use time_util::{DateTimeExt, TimeZoneExt};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    reuse_cert: bool,
    // #[arg(long)]
    // cert: PathBuf,
    // #[arg(long)]
    // key: PathBuf,
    // #[arg(long)]
    // token: String,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let token = std::env::var("JUSTMESSAGE_TELEGRAM_BOT_TOKEN").unwrap();
    let Args { reuse_cert } = Args::parse();
    let (pem_cert, pem_key, secret_token) = if reuse_cert {
        (
            std::fs::read_to_string("cert.pem").unwrap(),
            std::fs::read_to_string("key.pem").unwrap(),
            std::fs::read_to_string("secret-token").unwrap(),
        )
    } else {
        let certificate =
            rcgen::generate_simple_self_signed(["fr1.justmessage.uben.ovh".to_string()]).unwrap();
        let pem_cert = certificate.cert.pem();
        let pem_key = certificate.signing_key.serialize_pem();
        let secret_token = key_to_hex(gen_key());
        std::fs::write("cert.pem", &pem_cert).unwrap();
        std::fs::write("key.pem", &pem_key).unwrap();
        std::fs::write("secret-token", &secret_token).unwrap();
        (pem_cert, pem_key, secret_token)
    };

    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    if !reuse_cert {
        let response =
            telegram::set_webhook(&token, "https://fr1.justmessage.uben.ovh:8443".into())
                .drop_pending_updates()
                .certificate(pem_cert.clone().into())
                .secret_token(secret_token.clone())
                .send()
                .await
                .unwrap();
        println!("{response:#?}");
    }

    let (i_sender, i_receiver) = mpsc::channel::<Input>(8);
    let (o_sender, o_receiver) = mpsc::channel::<(Output, Context)>(8);

    let state = AppState::new();
    let processor = tokio::spawn(state.process_inputs(i_receiver, o_sender));
    let sender = tokio::spawn(sender(token.clone(), o_receiver));

    let app = Router::new()
        .route("/", post(handler))
        .with_state(i_sender)
        .layer(middleware::from_fn_with_state(
            HeaderValue::from_str(&secret_token).unwrap(),
            check_secret_token,
        ))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    let tls_conf = RustlsConfig::from_pem(pem_cert.into(), pem_key.into())
        .await
        .unwrap();
    axum_server::bind_rustls(([0, 0, 0, 0], 8443).into(), tls_conf)
        .serve(app.into_make_service())
        .await
        .unwrap();

    processor.await.unwrap();
    sender.await.unwrap();
}

async fn printer(payload: String) -> StatusCode {
    println!("{payload}");
    StatusCode::OK
}

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

async fn sender(token: String, mut receiver: Receiver<(Output, Context)>) {
    let renderer = Renderer::new();
    while let Some((output, context)) = receiver.recv().await {
        match output {
            Output::Ok => {
                telegram::send_text(&token, "ok".into(), context.chat)
                    .await
                    .unwrap();
            }
            Output::Failure => {
                telegram::send_text(&token, "fail".into(), context.chat)
                    .await
                    .unwrap();
            }
            Output::PleasePromoteTheBot => {
                let text = match context.language {
                    Language::En => "Please, promote me to administrator in the group settings.",
                    Language::Es => {
                        "Por favor, promocioneme administrador en la configuración del grupo."
                    }
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .await
                    .unwrap();
            }
            Output::YourAreNotPartOfAGroup => {
                let text = match context.language {
                    Language::En => "You are not part of a group.",
                    Language::Es => "No eres parte de une grupo.",
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .await
                    .unwrap();
            }
            Output::CouldNotRecognizeCommand => {
                let text = match context.language {
                    Language::En => "The command you wrote is not recognized.",
                    Language::Es => "El comando que escribiste no está reconocido.",
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .await
                    .unwrap();
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
                    .await
                    .unwrap();
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
                    .await
                    .unwrap();
            }
            Output::SpanOverrodeSpans(spans) => {
                use std::fmt::Write;
                let mut text = String::new();
                match context.language {
                    Language::En => writeln!(text, "The following time spans were overriden:"),
                    Language::Es => writeln!(text, "Se anularon los siguientes tramos de tiempo:"),
                }
                .unwrap();
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
                    .await
                    .unwrap();
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
                    .await
                    .unwrap();
            }
            Output::CouldNotInferMonth => {
                let text = match context.language {
                    Language::En => {
                        "I was not able to determine the month based on your indication."
                    }
                    Language::Es => "No era capaz de determinar el mes basandome en tu indicación.",
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .await
                    .unwrap();
            }
            Output::EnterOverrodeEntered(enter) => {
                let enter = context
                    .time_zone
                    .timestamp_opt(enter, 0)
                    .earliest()
                    .unwrap();

                let text = match context.language {
                    Language::En => "The previous entering time was overriden:",
                    Language::Es => "La hora de entrada previa se anuló:",
                };
                telegram::send_text(&token, format!("{text} {enter}"), context.chat)
                    .await
                    .unwrap();
            }
            Output::TryLeaveButNotEntered => {
                let text = match context.language {
                    Language::En => {
                        "You are trying to leave, but you did not enter in the first place."
                    }
                    Language::Es => "Estás tratando de salir, pero no entraste en primer lugar.",
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .await
                    .unwrap();
            }
            Output::Month {
                person,
                month,
                spans,
            } => {
                let month = context.time_zone.instant(month);

                let mut month = OutputMonth {
                    name: "Unknown".to_string(),
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
                telegram::send_photo(&token, image, context.chat)
                    .await
                    .unwrap();
            }
            Output::IAmNowAdministrator => {
                let text = match context.language {
                    Language::En => {
                        "I am now administrator in the group. I can now see who is part of the group."
                    }
                    Language::Es => {
                        "Ahora soy administrador en el grupo. Ahora puedo ver quién es parte del grupo."
                    }
                };
                telegram::send_text(&token, text.into(), context.chat)
                    .await
                    .unwrap();
            }
        }
    }
}
