use std::{ops::Deref, sync::{atomic::{AtomicUsize, Ordering}, OnceLock}};

use bytes::Bytes;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{digest::InvalidLength, Sha256};
use uuid::Uuid;
use zeromq::ZmqMessage;

use crate::CARGO_TOML;

pub struct KernelSession(OnceLock<String>);

impl KernelSession {
    pub const fn new() -> Self {
        KernelSession(OnceLock::new())
    }

    pub fn get(&self) -> &str {
        self.0.get_or_init(|| Uuid::new_v4().to_string())
    }
}

pub static KERNEL_SESSION: KernelSession = KernelSession::new();
pub static MESSAGE_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct Digester(OnceLock<Hmac<Sha256>>);

impl Digester {
    pub const fn new() -> Self {
        Digester(OnceLock::new())
    }

    pub fn key_init(&self, key: &[u8]) -> Result<(), InvalidLength> {
        self.0.set(Hmac::new_from_slice(key)?).expect("already set");
        Ok(())
    }

    pub fn get(&self) -> &Hmac<Sha256> {
        match self.0.get() {
            None => panic!("hmac not initialized"),
            Some(hmac) => hmac
        }
    }
}

pub static DIGESTER: Digester = Digester::new();

#[derive(Debug)]
pub struct Message {
    pub zmq_identities: Vec<String>,
    pub header: Header,
    pub parent_header: Option<Header>,
    pub metadata: Metadata,
    pub content: Content,
    pub buffers: Vec<Bytes>,
}

impl TryFrom<ZmqMessage> for Message {
    type Error = ();

    // TODO: add real error type here

    fn try_from(zmq_message: ZmqMessage) -> Result<Self, Self::Error> {
        let mut iter = zmq_message.into_vec().into_iter();

        let mut zmq_identities = Vec::new();
        while let Some(bytes) = iter.next() {
            if bytes.deref() == b"<IDS|MSG>" {
                break;
            }

            let id = String::from_utf8(bytes.into()).unwrap();
            zmq_identities.push(id);
        }

        let hmac_signature = iter.next().unwrap();
        let hmac_signature = String::from_utf8(hmac_signature.into()).unwrap();
        // TODO: verify signature

        let header = iter.next().unwrap();
        let header = std::str::from_utf8(&header).unwrap();
        let header: Header = serde_json::from_str(header).unwrap();

        let parent_header = iter.next().unwrap();
        let parent_header = std::str::from_utf8(&parent_header).unwrap();
        let parent_header: Option<Header> = match parent_header {
            "{}" => None,
            _ => serde_json::from_str(parent_header).unwrap(),
        };

        let metadata = iter.next().unwrap();
        let metadata = std::str::from_utf8(&metadata).unwrap();
        let metadata: Metadata = serde_json::from_str(metadata).unwrap();

        let content = iter.next().unwrap();
        let content = std::str::from_utf8(&content).unwrap();
        // TODO: add some handle to check from where the request came
        let content = ShellRequestMessage::parse_variant(&header.msg_type, content).unwrap();
        let content: Content = Content::ShellRequest(content);

        let buffers = iter.collect();

        Ok(Message {
            zmq_identities,
            header,
            parent_header,
            metadata,
            content,
            buffers,
        })
    }
}

impl TryFrom<Message> for ZmqMessage {
    // TODO: add real error type here
    type Error = ();

    fn try_from(msg: Message) -> Result<Self, Self::Error> {
        let header = serde_json::to_string(&msg.header).unwrap();
        let parent_header = match msg.parent_header {
            Some(ref parent_header) => serde_json::to_string(parent_header).unwrap(),
            None => "{}".to_owned()
        };
        let metadata = serde_json::to_string(&msg.metadata).unwrap();
        let Content::ShellReply(content) = msg.content
        else {
            panic!("tried to serialize not a reply");
        };
        let content = serde_json::to_string(&content).unwrap();
        let mut buffers = msg.buffers;

        let mut digester = DIGESTER.get().clone();
        digester.update(header.as_bytes());
        digester.update(parent_header.as_bytes());
        digester.update(metadata.as_bytes());
        digester.update(content.as_bytes());
        let signature = digester.finalize().into_bytes();
        let signature = hex::encode(signature);

        let mut bytes: Vec<Bytes> = Vec::new();

        for zmq_ids in msg.zmq_identities {
            bytes.push(zmq_ids.into())
        }

        bytes.push(Bytes::from_static(b"<IDS|MSG>"));
        bytes.push(signature.into());
        bytes.push(header.into());
        bytes.push(parent_header.into());
        bytes.push(metadata.into());
        bytes.push(content.into());
        bytes.append(&mut buffers);

        Ok(bytes.try_into().expect("only errors on empty vec"))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Header {
    pub msg_id: String,
    pub session: String,
    pub username: String,
    pub date: String,
    pub msg_type: String, // TODO: make this an enum
    pub version: String,
}

impl Header {
    pub fn new(msg_type: impl Into<String>) -> Self {
        let session = KERNEL_SESSION.get();
        let msg_counter = MESSAGE_COUNTER.fetch_add(1, Ordering::SeqCst);

        Header {
            msg_id: format!("{session}:{msg_counter}"),
            session: session.to_owned(),
            username: "nu".to_owned(),
            date: Utc::now().to_rfc3339(),
            msg_type: msg_type.into(),
            version: CARGO_TOML
                .package
                .metadata
                .jupyter
                .protocol_version
                .to_owned(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Metadata(serde_json::Value);

impl Metadata {
    pub fn empty() -> Self {
        Metadata(json!({}))
    }
}

#[derive(Debug)]
pub enum Content {
    ShellRequest(ShellRequestMessage),
    ShellReply(ShellReplyMessage),
}


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
            _ => todo!("unhandled request {variant}"),
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
    pub code: String,
    #[serde(default)]
    pub silent: bool,
    // TODO: check if this assertion can still be unhold or should be
    pub store_history: bool,
    // TODO: replace this with some kind of nu type
    pub user_expression: serde_json::Value,
    pub allow_stdin: bool,
    pub stop_on_error: bool,
}

#[derive(Debug, Serialize)]
pub struct ExecuteReply {
    pub execution_count: usize,
    pub user_expression: serde_json::Value,
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
