use axum::{Router, extract::Request, http::StatusCode, routing::post};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
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
    let generated =
        rcgen::generate_simple_self_signed(["fr1.justmessage.uben.ovh".to_string()]).unwrap();

    let response = telegram::set_webhook(&token, "https://fr1.justmessage.uben.ovh".into())
        .drop_pending_updates()
        .certificate(generated.cert.pem().into())
        .send()
        .await
        .unwrap();
    println!("{response:#?}");
    let tls_conf = RustlsConfig::from_pem(
        generated.cert.pem().into(),
        generated.signing_key.serialize_pem().into(),
    )
    .await
    .unwrap();
    let app = Router::new().route("/", post(handler));
    axum_server::bind_rustls(([0, 0, 0, 0], 443).into(), tls_conf)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler(request: Request) -> StatusCode {
    println!("{request:#?}");
    StatusCode::OK
}
