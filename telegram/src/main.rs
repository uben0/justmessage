use clap::Parser;
use frankenstein::{
    TelegramApi,
    client_ureq::Bot,
    input_file::InputFile,
    methods::{GetUpdatesParams, SendMessageParams, SendPhotoParams},
    types::AllowedUpdate,
    updates::UpdateContent,
};
use just_message::{JustMessage, Message, Response};
use lib_fichar::State as AppFichar;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::File,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct FrontTelegram {
    app_fichar: AppFichar,
    connexions: HashMap<i64, Connection>,
    acks: HashMap<u32, i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
enum Connection {
    AppFichar { person: u32 },
}

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    reset: bool,
    #[arg(short, long)]
    token: Option<String>,
}

fn main() {
    let Args { reset, token } = Args::parse();
    let token = token.unwrap_or_else(|| {
        dotenvy::dotenv().ok();
        std::env::var("JUSTMESSAGE_TELEGRAM_BOT_TOKEN").unwrap()
    });
    let mut state = if reset {
        FrontTelegram {
            app_fichar: AppFichar::default(),
            connexions: HashMap::new(),
            acks: HashMap::new(),
        }
    } else {
        symtree::from_reader(File::open("gen/telegram.scm").unwrap()).unwrap()
    };
    let bot = Bot::new(&token);
    let renderer = render::Renderer::new();
    let params = GetUpdatesParams::builder()
        .allowed_updates(Vec::from([AllowedUpdate::Message]))
        .build();
    let updates = bot.get_updates(&params).unwrap();
    for update in updates.result {
        if !state.acks.contains_key(&update.update_id) {
            state.acks.insert(update.update_id, now());

            match update.content {
                UpdateContent::Message(message) => {
                    let chat_id = message.chat.id;
                    let Connection::AppFichar { person } = *state
                        .connexions
                        .entry(chat_id)
                        .or_insert(Connection::AppFichar { person: 0 });

                    println!(
                        "{} > {}",
                        message.from.as_ref().unwrap().first_name,
                        message.text.as_ref().unwrap()
                    );
                    let responses = state.app_fichar.message(Message {
                        instant: message.date as i64,
                        content: message.text.unwrap(),
                        person,
                    });
                    for response in responses {
                        match response {
                            Response::Success => {
                                let params = SendMessageParams::builder()
                                    .chat_id(chat_id)
                                    .text("ok")
                                    .build();
                                bot.send_message(&params).unwrap();
                            }
                            Response::Text(text) => {
                                let params = SendMessageParams::builder()
                                    .chat_id(chat_id)
                                    .text(text)
                                    .build();
                                bot.send_message(&params).unwrap();
                            }
                            Response::Failure => {
                                let params = SendMessageParams::builder()
                                    .chat_id(chat_id)
                                    .text("err")
                                    .build();
                                bot.send_message(&params).unwrap();
                            }
                            Response::Document {
                                main,
                                bytes,
                                sources,
                            } => {
                                let image_png = renderer.render(main, sources, bytes);
                                std::fs::write("gen/tmp-img.png", image_png).unwrap();
                                let params = SendPhotoParams::builder()
                                    .chat_id(chat_id)
                                    .photo(InputFile {
                                        path: "gen/tmp-img.png".into(),
                                    })
                                    .build();
                                bot.send_photo(&params).unwrap();
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    symtree::to_writer_pretty(&state, File::create("gen/telegram.scm").unwrap()).unwrap();
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
