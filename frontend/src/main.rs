use aes_gcm::{
    AeadCore, Aes256Gcm, Key, KeyInit, Nonce,
    aead::{Aead, OsRng},
};
use axum::{Router, extract::State, routing::post};
use chrono_tz::Tz;
use clap::{Parser, Subcommand};
use hyper::StatusCode;
use just_message::{JustMessage, Language, Message as AppMessage, Response as AppResponse};
use lib_fichar::State as AppFichar;
use pbkdf2::pbkdf2_hmac_array;
use pest::Parser as _;
use render::Renderer;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use slab::Slab;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Write,
    str::FromStr,
};
use tokio::{
    net::TcpListener,
    sync::mpsc::{self, Receiver, Sender},
};
use tower_http::trace::{self, TraceLayer};
use tracing::{Level, info, warn};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    key: Option<String>,
    #[arg(long)]
    webhook: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Load,
    New {
        invitation: String,
        #[arg(long)]
        token: Option<String>,
    },
}

fn derive_key(key: &[u8]) -> [u8; 32] {
    pbkdf2_hmac_array::<Sha256, 32>(key, &[], 100_000)
}

#[tokio::main]
async fn main() {
    let Args {
        key,
        webhook,
        command,
    } = Args::parse();

    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();
    dotenvy::dotenv().ok();

    let key = key.unwrap_or_else(|| {
        std::env::var("JUSTMESSAGE_KEY").expect("key not set in environment variables")
    });
    let key = derive_key(key.as_bytes());
    info!("key derived");

    let state = match command {
        Command::Load => load_state(key),
        Command::New { token, invitation } => FrontState {
            admins: HashSet::new(),
            connections: HashMap::new(),
            instances: Slab::from_iter([(
                0,
                AppFichar::new(
                    "Atelier Bistrot".into(),
                    Tz::Europe__Madrid,
                    Language::Es,
                    ["Eddie".into(), "Gerbais".into()].into(),
                ),
            )]),
            invitations: HashMap::from([(
                invitation,
                Connection {
                    instance: 0,
                    person: 0,
                    admin: true,
                },
            )]),
            token: token.unwrap_or_else(|| {
                std::env::var("JUSTMESSAGE_TELEGRAM_BOT_TOKEN")
                    .expect("telegram bot token not set in environmnet variables")
            }),
        },
    };
    let token = state.token.clone();

    // let tls_config = ServerConfig::builder()
    //     .with_no_client_auth()
    //     .with_single_cert(
    //         Vec::from([cert.into()]),
    //         PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der())),
    //     )
    //     .unwrap();

    if webhook {
        // TODO: retry loop
        let response = telegram::set_webhook(&state.token, "fr1.justmessage.uben.ovh".into())
            .drop_pending_updates()
            .send()
            .await
            .unwrap();
        let status = response.status();
        if !status.is_success() {
            warn!("fail to set webhook {:?}", response.text().await.unwrap());
        }
        assert_eq!(status.as_u16(), 200);
    }

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
    let processor = tokio::spawn(process(key, state, receiver));
    axum::serve(tcp_listener, app)
        .with_graceful_shutdown(wait_terminate_signal())
        .await
        .unwrap();

    let state = processor.await.unwrap();

    if webhook {
        telegram::delete_webhook(&token).await.logged();
    }

    save_state(key, &state);

    info!("successful exit");
}

fn load_state(key: [u8; 32]) -> FrontState {
    let key = Key::<Aes256Gcm>::from(key);
    let cipher = Aes256Gcm::new(&key);

    let bytes = std::fs::read("state").unwrap();
    let nonce = Nonce::from_slice(&bytes[..12]);
    let bytes = cipher.decrypt(&nonce, &bytes[12..]).unwrap();
    postcard::from_bytes(&bytes).unwrap()
}
fn save_state(key: [u8; 32], state: &FrontState) {
    let key = Key::<Aes256Gcm>::from(key);
    let cipher = Aes256Gcm::new(&key);

    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    // let nonce = Nonce::from([0; 12]);
    assert_eq!(nonce.len(), 12);
    let bytes = postcard::to_allocvec(state).unwrap();
    let bytes = cipher.encrypt(&nonce, bytes.as_slice()).unwrap();
    let mut file = File::create("state").unwrap();
    file.write_all(&nonce).unwrap();
    file.write_all(&bytes).unwrap();
}

async fn wait_terminate_signal() {
    let mut termination = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("fail to install termination signal handle");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => (),
        _ = termination .recv() => (),
    }
    println!();
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
        println!("{update:#?}");
        sender.send(update).await.unwrap();
    } else {
        eprintln!("failed to parse body {body}");
    }
    StatusCode::OK
}

#[derive(pest_derive::Parser)]
#[grammar = "super_command.pest"]
struct SuperCommandParser;

#[derive(Debug, Clone)]
enum SuperCommand {
    Authenticate { key: String },
}
impl FromStr for SuperCommand {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Ok(mut pair) = SuperCommandParser::parse(Rule::command, s) else {
            return Err(());
        };
        let node = pair.next().unwrap().into_inner().next().unwrap();
        match node.as_rule() {
            Rule::command_auth => Ok(Self::Authenticate {
                key: node.into_inner().next().unwrap().as_str().to_string(),
            }),
            _ => unreachable!(),
        }
    }
}

async fn process(
    master_key: [u8; 32],
    mut state: FrontState,
    mut receiver: Receiver<Update>,
) -> FrontState {
    let renderer = Renderer::new();
    info!("listening for messages");
    while let Some(update) = receiver.recv().await {
        let chat_id = update.message.chat.id;

        if update.message.text.trim().starts_with('/') {
            match update.message.text.parse() {
                Err(_) => telegram::send_text(&state.token, "fail parsing".into(), chat_id)
                    .await
                    .logged(),
                Ok(SuperCommand::Authenticate { key }) => {
                    if derive_key(key.as_bytes()) == master_key {
                        state.admins.insert(Terminal::Telegram(chat_id));
                        telegram::send_text(&state.token, "you are now admin".into(), chat_id)
                            .await
                            .logged()
                    } else {
                        telegram::send_text(&state.token, "authentication failed".into(), chat_id)
                            .await
                            .logged()
                    }
                }
            }
        } else {
            match state.connections.get(&Terminal::Telegram(chat_id)) {
                None => match state.invitations.remove(update.message.text.trim()) {
                    Some(connection) => {
                        telegram::send_text(
                            &state.token,
                            format!(
                                "you joined {}",
                                state.instances[connection.instance as usize].name
                            ),
                            chat_id,
                        )
                        .await
                        .logged();
                        state
                            .connections
                            .insert(Terminal::Telegram(chat_id), connection);
                    }
                    None => {
                        telegram::send_text(&state.token, "unknown invitation".into(), chat_id)
                            .await
                            .logged();
                    }
                },
                Some(&Connection {
                    instance,
                    person,
                    admin: _,
                }) => {
                    let responses = state.instances[instance as usize].message(AppMessage {
                        instant: update.message.date,
                        content: update.message.text,
                        person,
                    });
                    for response in responses {
                        match response {
                            AppResponse::Success => {
                                telegram::send_text(&state.token, "ok".into(), chat_id)
                                    .await
                                    .logged();
                            }
                            AppResponse::Text(text) => {
                                telegram::send_text(&state.token, text, chat_id)
                                    .await
                                    .logged();
                            }
                            AppResponse::Failure => {
                                telegram::send_text(&state.token, "fail".into(), chat_id)
                                    .await
                                    .logged();
                            }
                            AppResponse::Document {
                                main,
                                bytes,
                                sources,
                            } => {
                                let image = renderer.render(main, sources, bytes);
                                telegram::send_photo(&state.token, image, update.message.chat.id)
                                    .await
                                    .logged();
                            }
                        }
                    }
                }
            }
        }
    }
    state
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

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Connection {
    instance: u32,
    person: u32,
    admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FrontState {
    admins: HashSet<Terminal>,
    connections: HashMap<Terminal, Connection>,
    instances: Slab<AppFichar>,
    invitations: HashMap<String, Connection>,
    token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
enum Terminal {
    Telegram(i64),
}
