use axum::{
    Json, Router,
    body::Body,
    extract::{Request, State, rejection::JsonRejection},
    http::{HeaderValue, Response, StatusCode},
    middleware::{self, Next},
    routing::post,
};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use fichar::{gen_key, key_to_hex};
use telegram::Update;
use tokio::sync::mpsc::{self, Sender};
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};
// use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    // #[arg(long)]
    // cert: PathBuf,
    // #[arg(long)]
    // key: PathBuf,
    #[arg(long)]
    token: String,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let token = std::env::var("JUSTMESSAGE_TELEGRAM_BOT_TOKEN").unwrap();
    // let Args { token } = Args::parse();
    let certificate =
        rcgen::generate_simple_self_signed(["fr1.justmessage.uben.ovh".to_string()]).unwrap();

    let secret_token = gen_key();
    let secret_token = key_to_hex(secret_token);

    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let response = telegram::set_webhook(&token, "https://fr1.justmessage.uben.ovh:8443".into())
        .drop_pending_updates()
        .certificate(certificate.cert.pem().into())
        .secret_token(secret_token.clone())
        .send()
        .await
        .unwrap();
    println!("{response:#?}");

    let (sender, mut receiver) = mpsc::channel::<Input>(8);

    tokio::spawn(async move {
        while let Some(update) = receiver.recv().await {
            todo!()
        }
    });

    let app = Router::new()
        .route("/", post(handler))
        .with_state(sender)
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

    let tls_conf = RustlsConfig::from_pem(
        certificate.cert.pem().into(),
        certificate.signing_key.serialize_pem().into(),
    )
    .await
    .unwrap();
    axum_server::bind_rustls(([0, 0, 0, 0], 8443).into(), tls_conf)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(
    sender: State<Sender<Input>>,
    payload: Result<Json<Update>, JsonRejection>,
) -> StatusCode {
    match payload {
        Ok(Json(update)) => {
            if let Ok(message) = Input::try_from(update) {
                sender.send(message).await.unwrap();
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

#[derive(Debug, Clone)]
enum Input {
    Text {
        chat: i64,
        person: i64,
        date: i64,
        text: String,
    },
}

impl TryFrom<Update> for Input {
    type Error = ();

    fn try_from(update: Update) -> Result<Self, Self::Error> {
        Ok(Self::Text {
            chat: update.message.chat.id,
            person: update.message.from.id,
            date: update.message.chat.id,
            text: update.message.text,
        })
    }
}
