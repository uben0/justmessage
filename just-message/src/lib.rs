// use lib_fichar::State as StateFichar;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub instant: i64,
    pub content: String,
    pub person: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Success,
    Text(String),
    Failure,
    Document {
        main: &'static str,
        bytes: HashMap<&'static str, Vec<u8>>,
        sources: HashMap<&'static str, String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub week_day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub offset: u32,
}

pub trait JustMessage: Serialize + Deserialize<'static> + Default {
    fn message(&mut self, message: Message) -> Vec<Response>;
    fn local_date_time(&self, instant: i64) -> LocalDateTime;
}

impl Response {
    pub fn err(err: &impl Debug) -> Vec<Self> {
        Vec::from([Self::Failure, Self::Text(format!("{:?}", err))])
    }
}
