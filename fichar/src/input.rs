use telegram::{ChatMember, ChatType, Update};

#[derive(Debug, Clone)]
pub enum Input {
    Text {
        chat: i64,
        group: bool,
        person: i64,
        date: i64,
        text: String,
    },
    NewGroup {
        chat: i64,
        name: String,
    },
    LeftChat {
        chat: i64,
        person: i64,
    },
    NowAdmin {
        chat: i64,
    },
}

impl TryFrom<Update> for Input {
    type Error = ();

    fn try_from(update: Update) -> Result<Self, Self::Error> {
        if let Some(message) = update.message {
            if let Some(text) = message.text {
                Ok(Self::Text {
                    chat: message.chat.id,
                    group: message.chat.kind == ChatType::Group,
                    person: message.from.id,
                    date: message.date,
                    text,
                })
            } else if message.group_chat_created {
                Ok(Self::NewGroup {
                    chat: message.chat.id,
                    name: message.chat.title.unwrap(),
                })
            } else {
                Err(())
            }
        } else if let Some(chat_member) = update.my_chat_member {
            if let ChatMember::Administrator { .. } = chat_member.new_chat_member {
                Ok(Self::NowAdmin {
                    chat: chat_member.chat.id,
                })
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }
}
