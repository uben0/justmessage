use reqwest::{
    Client, Error, RequestBuilder, Response,
    multipart::{Form, Part},
};

pub async fn send_photo(token: &str, photo: Vec<u8>, chat_id: i64) -> Result<Response, Error> {
    client(token, "sendPhoto")
        .multipart(
            Form::new()
                .part("chat_id", Part::text(format!("{}", chat_id)))
                .part("photo", Part::bytes(photo).file_name("month.png")),
        )
        .send()
        .await
}

pub async fn send_text(token: &str, text: String, chat_id: i64) -> Result<Response, Error> {
    client(token, "sendMessage")
        .multipart(
            Form::new()
                .part("chat_id", Part::text(format!("{}", chat_id)))
                .part("text", Part::text(text)),
        )
        .send()
        .await
}

pub fn set_webhook(token: &str, url: String) -> SetWebhook<'_> {
    SetWebhook {
        token,
        url,
        drop_pending_updates: false,
    }
}

pub struct SetWebhook<'a> {
    token: &'a str,
    url: String,
    drop_pending_updates: bool,
}
impl<'a> SetWebhook<'a> {
    pub fn drop_pending_updates(self) -> Self {
        Self {
            drop_pending_updates: true,
            ..self
        }
    }
    pub async fn send(self) -> Result<Response, Error> {
        client(self.token, "setWebhook")
            .multipart(Form::new().part("url", Part::text(self.url)).part(
                "drop_pending_updates",
                Part::text(if self.drop_pending_updates {
                    "True"
                } else {
                    "False"
                }),
            ))
            .send()
            .await
    }
}

pub async fn delete_webhook(token: &str) -> Result<Response, Error> {
    client(token, "deleteWebhook").send().await
}

fn client(token: &str, method: &str) -> RequestBuilder {
    Client::new().post(format!("https://api.telegram.org/bot{}/{}", token, method))
}
