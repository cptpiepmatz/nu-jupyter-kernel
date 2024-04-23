use std::io::{self, IsTerminal};

use serde::{Deserialize, Serialize};

use crate::CARGO_TOML;

#[derive(Debug, Deserialize)]
pub enum ShellRequest {
    Execute(ExecuteRequest),
    IsComplete(IsCompleteRequest),
    KernelInfo,
}

impl ShellRequest {
    pub fn parse_variant(variant: &str, body: &str) -> Result<Self, ()> {
        match variant {
            "execute_request" => return Ok(Self::Execute(serde_json::from_str(body).unwrap())),
            "is_complete_request" => {
                return Ok(Self::IsComplete(serde_json::from_str(body).unwrap()))
            }
            "kernel_info_request" if body == "{}" => return Ok(Self::KernelInfo),
            "kernel_info_request" => todo!("handle incorrect body here"),
            _ => {
                eprintln!("unknown request {variant}");
                return Err(());
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ShellReply {
    Ok(ShellReplyOk),
    Error {
        #[serde(alias = "ename")]
        name: String,
        #[serde(alias = "evalue")]
        value: String,
        traceback: Vec<String>,
    },
}

impl ShellReply {
    pub fn msg_type(request_type: &'_ str) -> Result<&'static str, ()> {
        Ok(match request_type {
            "execute_request" => "execute_reply",
            "is_complete_request" => "is_complete_reply",
            "kernel_info_request" => "kernel_info_reply",
            _ => todo!("handle unknown requests"),
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ShellReplyOk {
    Execute(ExecuteReply),
    KernelInfo(KernelInfoReply),
    IsComplete(IsCompleteReply),
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Serialize)]
pub struct ExecuteReply {
    pub execution_count: usize,
    pub user_expressions: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct IsCompleteRequest {
    pub code: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IsCompleteReply {
    Complete,
    Incomplete { indent: String },
    Invalid,
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct KernelInfoReply {
    pub protocol_version: String,
    pub implementation: String,
    pub implementation_version: String,
    pub language_info: KernelLanguageInfo,
    pub banner: String,
    pub debugger: bool,
    pub help_links: Vec<HelpLink>,
}

#[derive(Debug, Serialize)]
pub struct KernelLanguageInfo {
    pub name: String,
    pub version: String,
    pub mimetype: String,
    pub file_extension: String,
}

#[derive(Debug, Serialize)]
pub struct HelpLink {
    pub text: String,
    pub url: String,
}

impl<T, U> From<(T, U)> for HelpLink
where
    T: Into<String>,
    U: Into<String>,
{
    fn from(value: (T, U)) -> Self {
        HelpLink {
            text: value.0.into(),
            url: value.1.into(),
        }
    }
}

impl KernelInfoReply {
    pub fn get() -> Self {
        KernelInfoReply {
            protocol_version: CARGO_TOML
                .package
                .metadata
                .jupyter
                .protocol_version
                .to_owned(),
            implementation: CARGO_TOML.package.name.to_owned(),
            implementation_version: CARGO_TOML.package.version.to_owned(),
            language_info: KernelLanguageInfo {
                name: "nushell".to_owned(),
                version: CARGO_TOML.dependencies.nu_engine.version.to_owned(),
                // TODO: verify this
                mimetype: "text/nu".to_owned(),
                file_extension: ".nu".to_owned(),
            },
            banner: include_str!("../../../banner.txt").to_owned(),
            debugger: false,
            help_links: [
                ("Discord", "https://discord.gg/NtAbbGn"),
                ("GitHub", "https://github.com/nushell/nushell"),
            ]
            .into_iter()
            .map(|pair| pair.into())
            .collect(),
        }
    }
}
