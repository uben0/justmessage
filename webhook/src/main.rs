use clap::{Parser, Subcommand};
use frankenstein::{
    TelegramApi, client_ureq::Bot, methods::SetWebhookParams, types::AllowedUpdate,
};

#[derive(Parser)]
struct Args {
    #[arg(long, short)]
    token: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, Subcommand)]
enum Command {
    Enable,
    Disable,
}

// TODO: try frankenstein to setup webhook with self-signed cert

fn main() {
    let Args { token, command } = Args::parse();
    let token = token.unwrap_or_else(|| {
        dotenvy::dotenv().ok();
        std::env::var("JUSTMESSAGE_TELEGRAM_BOT_TOKEN").unwrap()
    });
    let bot = Bot::new(&token);

    let url = match command {
        Command::Enable => "https://fr1.justmessage.uben.ovh",
        Command::Disable => "",
    };
    let params = SetWebhookParams::builder()
        .url(url)
        .allowed_updates(Vec::from([AllowedUpdate::Message]))
        .build();
    bot.set_webhook(&params).unwrap();
}
