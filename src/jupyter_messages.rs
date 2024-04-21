use serde::{Deserialize, Serialize};

use crate::CARGO_TOML;

pub enum ShellMessage {
    Request(ShellRequestMessage),
    Reply(ShellReplyMessage),
}

#[derive(Debug, Deserialize)]
pub enum ShellRequestMessage {
    Execute(ExecuteRequest),
    KernelInfo,
}

impl ShellRequestMessage {
    pub fn parse_variant(variant: &str, body: &str) -> Result<Self, ()> {
        match variant {
            "execute_request" => return Ok(Self::Execute(serde_json::from_str(body).unwrap())),
            "kernel_info_request" if body == "{}" => return Ok(Self::KernelInfo),
            "kernel_info_request" => todo!("handle incorrect body here"),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ShellReplyMessage {
    Ok(ShellReplyOk),
    Error {
        #[serde(alias = "ename")]
        name: String,
        #[serde(alias = "evalue")]
        value: String,
        traceback: Vec<String>,
    },
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ShellReplyOk {
    Execute(ExecuteReply),
    KernelInfo(KernelInfoReply),
}

#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    code: String,
    #[serde(default)]
    silent: bool,
    // TODO: check if this assertion can still be unhold or should be
    store_history: bool,
    // TODO: replace this with some kind of nu type
    user_expression: serde_json::Value,
    allow_stdin: bool,
    stop_on_error: bool,
}

#[derive(Debug, Serialize)]
pub struct ExecuteReply {
    execution_count: usize,
    user_expression: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct KernelInfoReply {
    protocol_version: String,
    implementation: String,
    implementation_version: String,
    language_info: KernelLanguageInfo,
    banner: String,
    debugger: bool,
    help_links: Vec<HelpLink>,
}

#[derive(Debug, Serialize)]
pub struct KernelLanguageInfo {
    name: String,
    version: String,
    mimetype: String,
    file_extension: String,
}

#[derive(Debug, Serialize)]
pub struct HelpLink {
    text: String,
    url: String,
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
            banner: include_str!("../banner.txt").to_owned(),
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
