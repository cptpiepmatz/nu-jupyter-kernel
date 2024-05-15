use std::collections::HashMap;

use derive_more::From;
use serde::Serialize;
use strum::AsRefStr;

use super::Header;
use crate::jupyter::messages::{Message, Metadata};

#[derive(Debug, Serialize, From, Clone)]
#[serde(untagged)]
pub enum IopubBroacast {
    Stream(Stream),
    DisplayData(DisplayData),
    UpdateDisplayData,
    ExecuteInput,
    ExecuteResult(ExecuteResult),
    Error(Error),
    Status(Status),
    ClearOutput,
    DebugEvent,
}

impl IopubBroacast {
    pub fn msg_type(&self) -> &'static str {
        match self {
            IopubBroacast::Stream(_) => "stream",
            IopubBroacast::DisplayData(_) => "display_data",
            IopubBroacast::UpdateDisplayData => "update_display_data",
            IopubBroacast::ExecuteInput => "execute_input",
            IopubBroacast::ExecuteResult(_) => "execute_result",
            IopubBroacast::Error(_) => "error",
            IopubBroacast::Status(_) => "status",
            IopubBroacast::ClearOutput => "clear_output",
            IopubBroacast::DebugEvent => "debug_event",
        }
    }
}

#[derive(Debug, Serialize, Clone, Copy, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum StreamName {
    Stdout,
    Stderr,
}

#[derive(Debug, Serialize, Clone)]
pub struct Stream {
    pub name: StreamName,
    pub text: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DisplayData {
    pub data: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
    pub transient: HashMap<String, String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ExecuteResult {
    pub execution_count: usize,
    pub data: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Error {
    #[serde(rename = "ename")]
    pub name: String,

    #[serde(rename = "evalue")]
    pub value: String,

    pub traceback: Vec<String>,
}

#[derive(Debug, Serialize, Clone, Copy)]
#[serde(tag = "execution_state", rename_all = "snake_case")]
pub enum Status {
    Busy,
    Idle,
}

impl Status {
    pub fn into_message(self, parent_header: impl Into<Option<Header>>) -> Message<IopubBroacast> {
        let broadcast = IopubBroacast::Status(self);
        let msg_type = broadcast.msg_type();
        Message {
            zmq_identities: vec![],
            header: Header::new(msg_type),
            parent_header: parent_header.into(),
            metadata: Metadata::empty(),
            content: broadcast,
            buffers: vec![],
        }
    }
}
