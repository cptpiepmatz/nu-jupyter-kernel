use serde::{Deserialize, Serialize};

use crate::jupyter::kernel_info::KernelInfo;

#[derive(Debug, Deserialize, Clone)]
pub enum ShellRequest {
    Execute(ExecuteRequest),
    IsComplete(IsCompleteRequest),
    KernelInfo,
}

impl ShellRequest {
    pub fn parse_variant(variant: &str, body: &str) -> Result<Self, ()> {
        match variant {
            "execute_request" => Ok(Self::Execute(serde_json::from_str(body).unwrap())),
            "is_complete_request" => Ok(Self::IsComplete(serde_json::from_str(body).unwrap())),
            "kernel_info_request" => Ok(Self::KernelInfo),
            _ => {
                eprintln!("unknown request {variant}");
                Err(())
            }
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ShellReply {
    Ok(ShellReplyOk),
    Error {
        #[serde(rename = "ename")]
        name: String,
        #[serde(rename = "evalue")]
        value: String,
        traceback: Vec<String>,
    },
}

impl ShellReply {
    pub fn msg_type(request_type: &'_ str) -> Result<&'static str, ()> {
        Ok(match request_type {
            "kernel_info_request" => "kernel_info_reply",
            "execute_request" => "execute_reply",
            "is_complete_request" => "is_complete_reply",
            _ => todo!("handle unknown requests"),
        })
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(untagged)]
pub enum ShellReplyOk {
    KernelInfo(KernelInfo),
    Execute(ExecuteReply),
    IsComplete(IsCompleteReply),
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExecuteRequest {
    pub code: String,
    #[serde(default)]
    pub silent: bool,
    // TODO: check if this assertion can still be unhold or should be
    pub store_history: bool,
    // TODO: figure out what to do with this
    pub user_expressions: serde_json::Value,
    pub allow_stdin: bool,
    pub stop_on_error: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct ExecuteReply {
    pub execution_count: usize,
    pub user_expressions: serde_json::Value,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IsCompleteRequest {
    pub code: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IsCompleteReply {
    Complete,
    Incomplete { indent: String },
    Invalid,
    Unknown,
}
