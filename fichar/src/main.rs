use axum::{Router, extract::Request, http::StatusCode, routing::post};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    cert: PathBuf,
    #[arg(long)]
    key: PathBuf,
    #[arg(long)]
    token: String,
}

#[tokio::main]
async fn main() {
    let Args { cert, key, token } = Args::parse();
    let response = telegram::set_webhook(&token, "https://fr1.justmessage.uben.ovh".into())
        .drop_pending_updates()
        .certificate(std::fs::read(&cert).unwrap())
        .send()
        .await
        .unwrap();
    println!("{response:#?}");
    let tls_conf = RustlsConfig::from_pem_file(cert, key).await.unwrap();
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
