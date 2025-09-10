use reqwest::{
    Client, Error, RequestBuilder, Response,
    multipart::{Form, Part},
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Update {
    pub update_id: u64,
    #[serde(default)]
    pub message: Option<Message>,
    pub my_chat_member: Option<ChatMemberUpdated>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Message {
    pub message_id: i32,
    pub from: User,
    pub chat: Chat,
    pub date: i64,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub group_chat_created: bool,
    #[serde(default)]
    pub left_chat_member: Option<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct User {
    pub id: i64,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub language_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Chat {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: ChatType,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ChatMemberUpdated {
    pub chat: Chat,
    pub from: User,
    pub date: i64,
    pub old_chat_member: ChatMember,
    pub new_chat_member: ChatMember,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "status")]
pub enum ChatMember {
    #[serde(rename = "creator")]
    Owner { user: User },
    #[serde(rename = "administrator")]
    Administrator { user: User },
    #[serde(rename = "member")]
    Member { user: User },
    #[serde(rename = "restricted")]
    Restricted { user: User },
    #[serde(rename = "left")]
    Left { user: User },
    #[serde(rename = "kicked")]
    Banned { user: User },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChatType {
    #[serde(rename = "private")]
    Private,
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "supergroup")]
    SuperGroup,
    #[serde(rename = "channel")]
    Channel,
}

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

pub async fn send_document(
    token: &str,
    document: Vec<u8>,
    chat_id: i64,
) -> Result<Response, Error> {
    client(token, "sendDocument")
        .multipart(
            Form::new()
                .part("chat_id", Part::text(format!("{}", chat_id)))
                .part("document", Part::bytes(document).file_name("month.pdf")),
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

pub async fn send_markdown(token: &str, text: String, chat_id: i64) -> Result<Response, Error> {
    client(token, "sendMessage")
        .multipart(
            Form::new()
                .part("chat_id", Part::text(format!("{}", chat_id)))
                .part("text", Part::text(text))
                .part("parse_mode", Part::text("MarkdownV2")),
        )
        .send()
        .await
}

pub fn set_webhook(token: &str, url: String) -> SetWebhook<'_> {
    SetWebhook {
        token,
        url,
        drop_pending_updates: false,
        allowed_updates: Vec::new(),
        certificate: None,
        secret_token: None,
    }
}

pub struct SetWebhook<'a> {
    token: &'a str,
    url: String,
    allowed_updates: Vec<String>,
    drop_pending_updates: bool,
    certificate: Option<Vec<u8>>,
    secret_token: Option<String>,
}
impl<'a> SetWebhook<'a> {
    pub fn allowed_update(mut self, allowed_update: &str) -> Self {
        self.allowed_updates.push(allowed_update.to_string());
        self
    }
    pub fn certificate(self, certificate: Vec<u8>) -> Self {
        Self {
            certificate: Some(certificate),
            ..self
        }
    }
    pub fn drop_pending_updates(self) -> Self {
        Self {
            drop_pending_updates: true,
            ..self
        }
    }
    pub fn secret_token(self, secret_token: String) -> Self {
        Self {
            secret_token: Some(secret_token),
            ..self
        }
    }
    pub async fn send(self) -> Result<Response, Error> {
        client(self.token, "setWebhook")
            .multipart(
                Form::new()
                    .part("url", Part::text(self.url))
                    .part(
                        "allowed_updates",
                        Part::text(serde_json::to_string(&self.allowed_updates).unwrap()),
                    )
                    .part_opt(
                        "drop_pending_updates",
                        self.drop_pending_updates.then(|| Part::text("True")),
                    )
                    .part_opt(
                        "certificate",
                        self.certificate
                            .map(|cert| Part::bytes(cert).file_name("cert.pem")),
                    )
                    .part_opt(
                        "secret_token",
                        self.secret_token.map(|token| Part::text(token)),
                    ),
            )
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

trait FormExt {
    fn part_opt<T>(self, name: T, part: Option<Part>) -> Self
    where
        T: Into<Cow<'static, str>>;
}
impl FormExt for Form {
    fn part_opt<T>(self, name: T, part: Option<Part>) -> Self
    where
        T: Into<Cow<'static, str>>,
    {
        match part {
            Some(part) => self.part(name, part),
            None => self,
        }
    }
}
