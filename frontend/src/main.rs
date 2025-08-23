use axum::{Router, extract::State, routing::post};
use clap::Parser;
use hyper::StatusCode;
use just_message::{JustMessage, Message as AppMessage, Response as AppResponse};
use lib_fichar::State as AppFichar;
use render::Renderer;
use serde::{Deserialize, Serialize};
use slab::Slab;
use std::collections::HashMap;
use tokio::{
    net::TcpListener,
    sync::mpsc::{self, Receiver, Sender},
};
use tower_http::trace::{self, TraceLayer};
use tracing::{Level, info, warn};

#[derive(Parser)]
struct Args {
    invitation: String,
    #[arg(long)]
    token: Option<String>,
}

#[tokio::main]
async fn main() {
    let Args { token, invitation } = Args::parse();
    let token = token.unwrap_or_else(|| {
        dotenvy::dotenv().ok();
        std::env::var("JUSTMESSAGE_TELEGRAM_BOT_TOKEN").unwrap()
    });

    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();
    // let tls_config = ServerConfig::builder()
    //     .with_no_client_auth()
    //     .with_single_cert(
    //         Vec::from([cert.into()]),
    //         PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der())),
    //     )
    //     .unwrap();

    let response = telegram::set_webhook(&token, "fr1.justmessage.uben.ovh".into())
        .drop_pending_updates()
        .send()
        .await
        .unwrap();
    let status = response.status();
    println!("{:#?}", response.text().await.unwrap());
    assert_eq!(status.as_u16(), 200);

    let tcp_listener = TcpListener::bind("[::1]:8000").await.unwrap();

    let (sender, receiver) = mpsc::channel(64);
    let app = Router::new()
        .route("/", post(handler))
        .with_state(sender)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        );
    let processor = tokio::spawn(process(token.clone(), receiver, invitation));
    axum::serve(tcp_listener, app)
        .with_graceful_shutdown(wait_terminate_signal())
        .await
        .unwrap();

    processor.await.unwrap();

    telegram::delete_webhook(&token).await.logged();
    info!("successful exit");
}

async fn wait_terminate_signal() {
    let mut termination = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("fail to install termination signal handle");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => (),
        _ = termination .recv() => (),
    }
    info!("received termination signal");
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct Update {
    update_id: u64,
    message: Message,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct Message {
    message_id: i32,
    from: Person,
    chat: Chat,
    date: i64,
    text: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct Person {
    id: i64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct Chat {
    id: i64,
}

async fn handler(sender: State<Sender<Update>>, body: String) -> StatusCode {
    if let Ok(update) = serde_json::from_str(&body) {
        println!("{:#?}", update);
        sender.send(update).await.unwrap();
    } else {
        eprintln!("failed to parse body {}", body);
    }
    StatusCode::OK
}

async fn process(token: String, mut receiver: Receiver<Update>, invitation: String) {
    let mut state = FrontState {
        connections: HashMap::new(),
        instances: Slab::from_iter([(0, AppFichar::default())]),
        invitations: HashMap::from([(
            invitation,
            Connection {
                instance: 0,
                person: 0,
                admin: true,
            },
        )]),
    };
    let renderer = Renderer::new();
    while let Some(update) = receiver.recv().await {
        let chat_id = update.message.chat.id;
        match state.connections.get(&update.message.chat.id) {
            None => match state.invitations.get(update.message.text.trim()) {
                Some(&connection) => {
                    state.connections.insert(update.message.chat.id, connection);
                }
                None => {
                    println!("unknown invitation");
                }
            },
            Some(&Connection {
                instance,
                person,
                admin: _,
            }) => {
                if update.message.text.trim() == "reset" {
                    state.instances[instance as usize] = AppFichar::default();
                    continue;
                }
                let responses = state.instances[instance as usize].message(AppMessage {
                    instant: update.message.date,
                    content: update.message.text,
                    person: person,
                });
                for response in responses {
                    match response {
                        AppResponse::Success => {
                            telegram::send_text(&token, "ok".into(), chat_id)
                                .await
                                .logged();
                        }
                        AppResponse::Text(text) => {
                            telegram::send_text(&token, text, chat_id).await.logged();
                        }
                        AppResponse::Failure => {
                            telegram::send_text(&token, "fail".into(), chat_id)
                                .await
                                .logged();
                        }
                        AppResponse::Document {
                            main,
                            bytes,
                            sources,
                        } => {
                            let image = renderer.render(main, sources, bytes);
                            telegram::send_photo(&token, image, update.message.chat.id)
                                .await
                                .logged();
                        }
                    }
                }
            }
        }
    }
}

trait Logged {
    fn logged(self);
}
impl Logged for Result<reqwest::Response, reqwest::Error> {
    fn logged(self) {
        match self {
            Ok(response) => {
                if !response.status().is_success() {
                    warn!("fail sending request: {}", response.status());
                }
            }
            Err(error) => warn!("fail sending request: {}", error),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
struct Connection {
    instance: u32,
    person: u32,
    admin: bool,
}

#[derive(Debug, Clone)]
struct FrontState {
    connections: HashMap<i64, Connection>,
    instances: Slab<AppFichar>,
    invitations: HashMap<String, Connection>,
}
