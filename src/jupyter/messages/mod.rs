use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use bytes::Bytes;
use chrono::Utc;
use derive_more::From;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::digest::InvalidLength;
use sha2::Sha256;
use uuid::Uuid;
use zmq::{Socket, DONTWAIT};

use self::shell::ShellRequest;
use crate::{Channel, CARGO_TOML};

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

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Metadata(serde_json::Value);

impl Metadata {
    pub fn empty() -> Self {
        Metadata(json!({}))
    }
}

#[derive(Debug, Deserialize)]
pub enum IncomingContent {
    Shell(shell::ShellRequest),
}

#[derive(Debug, Serialize, From)]
pub enum OutgoingContent {
    Shell(shell::ShellReply),
    Iopub(iopub::IopubBroacast),
}

#[derive(Debug)]
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
    fn recv(socket: &Socket, source: Channel) -> Result<Self, ()> {
        let mut zmq_identities = Vec::new();
        loop {
            let bytes = socket.recv_bytes(ZMQ_WAIT).unwrap();
            if &bytes == b"<IDS|MSG>" {
                break;
            }
            zmq_identities.push(Bytes::from(bytes));
        }

        let signature = socket.recv_string(ZMQ_WAIT).unwrap().unwrap();

        let header = socket.recv_string(ZMQ_WAIT).unwrap().unwrap();
        let header: Header = serde_json::from_str(&header).unwrap();

        let parent_header = socket.recv_string(ZMQ_WAIT).unwrap().unwrap();
        let parent_header: Option<Header> = match parent_header.as_str() {
            "{}" => None,
            ph => serde_json::from_str(ph).unwrap(),
        };

        let metadata = socket.recv_string(ZMQ_WAIT).unwrap().unwrap();
        let metadata: Metadata = serde_json::from_str(&metadata).unwrap();

        let content = socket.recv_string(ZMQ_WAIT).unwrap().unwrap();
        let content = match source {
            Channel::Shell => {
                IncomingContent::Shell(ShellRequest::parse_variant(&header.msg_type, &content)?)
            }
            Channel::Iopub => unreachable!("only outgoing"),
            Channel::Stdin => todo!(),
            Channel::Control => todo!(),
        };

        let buffers: Vec<Bytes> = match socket.recv_multipart(DONTWAIT) {
            Ok(buffers) => buffers.into_iter().map(Bytes::from).collect(),
            Err(zmq::Error::EAGAIN) => vec![],
            Err(_) => todo!(),
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

impl Message<ShellRequest> {
    pub fn recv(socket: &Socket) -> Result<Self, ()> {
        let msg = Message::<IncomingContent>::recv(socket, Channel::Shell)?;
        let Message {
            zmq_identities,
            header,
            parent_header,
            metadata,
            content,
            buffers,
        } = msg;
        let IncomingContent::Shell(content) = content;
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
