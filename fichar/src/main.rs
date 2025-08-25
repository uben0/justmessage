use axum::{Router, extract::Request, http::StatusCode, routing::post};
use axum_server::tls_rustls::RustlsConfig;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let token = std::env::var("JUSTMESSAGE_TELEGRAM_BOT_TOKEN").unwrap();
    let response = telegram::set_webhook(&token, "https://fr1.justmessage.uben.ovh".into())
        .drop_pending_updates()
        .certificate(std::fs::read("../certy/YOURPUBLIC.pem").unwrap())
        .send()
        .await
        .unwrap();
    println!("{response:#?}");
    let tls_conf =
        RustlsConfig::from_pem_file("../certy/YOURPUBLIC.pem", "../certy/YOURPRIVATE.pem")
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
