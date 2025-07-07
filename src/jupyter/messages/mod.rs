use std::ops::Deref;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use bytes::Bytes;
use chrono::Utc;
use control::ControlRequest;
use derive_more::From;
use hmac::{Hmac, Mac};
use nu_protocol::{FromValue, IntoValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;
use sha2::digest::InvalidLength;
use uuid::Uuid;
use zeromq::SocketRecv;

use self::shell::ShellRequest;
use crate::{CARGO_TOML, Channel};

pub mod control;
pub mod iopub;
pub mod multipart;
pub mod shell;

pub static KERNEL_SESSION: KernelSession = KernelSession::new();
pub static MESSAGE_COUNTER: AtomicUsize = AtomicUsize::new(0);
pub static DIGESTER: Digester = Digester::new();

pub struct KernelSession(OnceLock<String>);

impl KernelSession {
    pub const fn new() -> Self {
        KernelSession(OnceLock::new())
    }

    pub fn get(&self) -> &str {
        self.0.get_or_init(|| Uuid::new_v4().to_string())
    }
}

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
            Some(hmac) => hmac,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, IntoValue, FromValue)]
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
            username: "nu_kernel".to_owned(),
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Metadata(serde_json::Value);

impl Metadata {
    pub fn empty() -> Self {
        Metadata(json!({}))
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum IncomingContent {
    Shell(shell::ShellRequest),
    Control(control::ControlRequest),
}

#[derive(Debug, Serialize, From, Clone)]
pub enum OutgoingContent {
    Shell(shell::ShellReply),
    Iopub(iopub::IopubBroacast),
    Control(control::ControlReply),
}

#[derive(Debug, Clone)]
pub struct Message<C> {
    pub zmq_identities: Vec<Bytes>,
    pub header: Header,
    pub parent_header: Option<Header>,
    pub metadata: Metadata,
    pub content: C,
    pub buffers: Vec<Bytes>,
}

static ZMQ_WAIT: i32 = 0;

impl Message<IncomingContent> {
    // TODO: add a real error type here
    async fn recv<S: SocketRecv>(socket: &mut S, source: Channel) -> Result<Self, ()> {
        let mut zmq_message = socket.recv().await.unwrap().into_vec().into_iter();
        let zmq_message = &mut zmq_message;

        let mut zmq_identities = Vec::new();
        for bytes in zmq_message.by_ref() {
            if bytes.deref() == b"<IDS|MSG>" {
                break;
            }
            zmq_identities.push(bytes.to_owned());
        }

        // TODO: add error handling for this here
        fn next_string(byte_iter: &mut impl Iterator<Item = Bytes>) -> String {
            String::from_utf8(byte_iter.next().unwrap().to_vec()).unwrap()
        }

        let signature = next_string(zmq_message);

        let header = next_string(zmq_message);
        let header: Header = serde_json::from_str(&header).unwrap();

        let parent_header = next_string(zmq_message);
        let parent_header: Option<Header> = match parent_header.as_str() {
            "{}" => None,
            ph => serde_json::from_str(ph).unwrap(),
        };

        let metadata = next_string(zmq_message);
        let metadata: Metadata = serde_json::from_str(&metadata).unwrap();

        let content = next_string(zmq_message);
        // FIXME: this is a annoying solution, should be handled somehow by the type
        // system better
        let content = match source {
            Channel::Shell => {
                IncomingContent::Shell(ShellRequest::parse_variant(&header.msg_type, &content)?)
            }
            Channel::Stdin => todo!(),
            Channel::Control => {
                IncomingContent::Control(ControlRequest::parse_variant(&header.msg_type, &content)?)
            }
        };

        let buffers: Vec<Bytes> = zmq_message.collect();

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

impl Message<ShellRequest> {
    pub async fn recv<S: SocketRecv>(socket: &mut S) -> Result<Self, ()> {
        let msg = Message::<IncomingContent>::recv(socket, Channel::Shell).await?;
        let Message {
            zmq_identities,
            header,
            parent_header,
            metadata,
            content,
            buffers,
        } = msg;
        let IncomingContent::Shell(content) = content
        else {
            panic!("unexpected content");
        };
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

impl Message<ControlRequest> {
    pub async fn recv<S: SocketRecv>(socket: &mut S) -> Result<Self, ()> {
        let msg = Message::<IncomingContent>::recv(socket, Channel::Control).await?;
        let Message {
            zmq_identities,
            header,
            parent_header,
            metadata,
            content,
            buffers,
        } = msg;
        let IncomingContent::Control(content) = content
        else {
            panic!("unexpected content");
        };
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
