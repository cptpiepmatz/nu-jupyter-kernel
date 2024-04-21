// for now
#![allow(dead_code)]
#![allow(unused_variables)]

use std::borrow::Cow;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use bytes::Bytes;
use chrono::Utc;
use clap::{Parser, Subcommand};
use connection_file::ConnectionFile;
use const_format::formatcp;
use hmac::{Hmac, Mac};
use jupyter_messages::{ShellMessage, ShellReplyMessage, ShellRequestMessage};
use parking_lot::Mutex;
use register_kernel::{register_kernel, RegisterLocation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::digest::InvalidLength;
use sha2::Sha256;
use tokio::select;
use uuid::Uuid;
use zeromq::{PubSocket, RepSocket, RouterSocket, Socket, SocketRecv, SocketSend, ZmqMessage};

use crate::jupyter_messages::{KernelInfoReply, ShellReplyOk};

mod connection_file;
mod execute_nu;
mod jupyter_messages;
mod register_kernel;

static_toml::static_toml! {
    const CARGO_TOML = include_toml!("Cargo.toml");
}

struct KernelSession(OnceLock<String>);

impl KernelSession {
    const fn new() -> Self {
        KernelSession(OnceLock::new())
    }

    fn get(&self) -> &str {
        self.0.get_or_init(|| Uuid::new_v4().to_string())
    }
}

static KERNEL_SESSION: KernelSession = KernelSession::new();
static MESSAGE_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Digester(OnceLock<Hmac<Sha256>>);

impl Digester {
    const fn new() -> Self {
        Digester(OnceLock::new())
    }

    fn key_init(&self, key: &[u8]) -> Result<(), InvalidLength> {
        self.0.set(Hmac::new_from_slice(key)?).expect("already set");
        Ok(())
    }

    fn get(&self) -> &Hmac<Sha256> {
        match self.0.get() {
            None => panic!("hmac not initialized"),
            Some(hmac) => hmac
        }
    }
}

static DIGESTER: Digester = Digester::new();

#[derive(Debug, Parser)]
#[command(version, long_version = formatcp!(
    "{}\nnu-engine {}",
    CARGO_TOML.package.version,
    CARGO_TOML.dependencies.nu_engine.version
))]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Register {
        #[clap(long, group = "location")]
        user: bool,

        #[clap(long, group = "location")]
        system: bool,
    },

    Start {
        connection_file_path: PathBuf,
    },
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    match args.command {
        Command::Register { user, system } => {
            let location = match (user, system) {
                (true, true) => unreachable!("handled by clap"),
                (false, true) => RegisterLocation::System,
                (true, false) => RegisterLocation::User,
                (false, false) => RegisterLocation::User, // default case
            };
            register_kernel(location);
        }
        Command::Start {
            connection_file_path,
        } => start_kernel(connection_file_path).await,
    }
}

async fn start_kernel(connection_file_path: impl AsRef<Path>) {
    let connection_file = ConnectionFile::from_path(connection_file_path).unwrap();

    DIGESTER.key_init(&connection_file.key).unwrap();

    let endpoint = |port| format!("{}://127.0.0.1:{}", connection_file.transport, port);

    let shell_endpoint = endpoint(connection_file.shell_port);
    let mut shell_socket = RouterSocket::new();
    let shell_endpoint = shell_socket.bind(&shell_endpoint).await.unwrap();

    let iopub_endpoint = endpoint(connection_file.iopub_port);
    let mut iopub_socket = PubSocket::new();
    let iopub_endpoint = iopub_socket.bind(&iopub_endpoint).await.unwrap();

    let stdin_endpoint = endpoint(connection_file.stdin_port);
    let mut stdin_socket = RouterSocket::new();
    let stdin_endpoint = stdin_socket.bind(&stdin_endpoint).await.unwrap();

    let control_endpoint = endpoint(connection_file.control_port);
    let mut control_socket = RouterSocket::new();
    let control_endpoint = control_socket.bind(&control_endpoint).await.unwrap();

    let heartbeat_endpoint = endpoint(connection_file.heartbeat_port);
    let mut heartbeat_socket = RepSocket::new();
    let heartbeat_endpoint = heartbeat_socket.bind(&heartbeat_endpoint).await.unwrap();

    let iopub_socket = Mutex::new(iopub_socket);

    println!("start listening on sockets");

    loop {
        select! {
            m = shell_socket.recv() => handle_shell(m.unwrap(), &mut shell_socket, &iopub_socket).await,
            m = stdin_socket.recv() => handle_stdin(m.unwrap(), &mut stdin_socket, &iopub_socket).await,
            m = control_socket.recv() => handle_control(m.unwrap(), &mut control_socket, &iopub_socket).await,
            m = heartbeat_socket.recv() => handle_heartbeat(m.unwrap(), &mut heartbeat_socket).await,
        }
    }
}

async fn handle_shell(message: ZmqMessage, socket: &mut RouterSocket, iopub: &Mutex<PubSocket>) {
    dbg!(("shell", &message));
    let message = Message::try_from(message).unwrap();
    dbg!(&message);
    match message.content {
        Content::ShellRequest(ShellRequestMessage::KernelInfo) => {
            let session = KERNEL_SESSION.get();
            let reply = KernelInfoReply::get();
            let reply = Message {
                zmq_identities: message.zmq_identities,
                header: Header::new("kernel_info_reply"),
                parent_header: Some(message.header),
                metadata: Metadata::empty(),
                content: Content::ShellReply(ShellReplyMessage::Ok(ShellReplyOk::KernelInfo(
                    KernelInfoReply::get(),
                ))),
                buffers: vec![],
            };
            dbg!(&reply);
            let reply = reply.try_into().unwrap();
            dbg!(&reply);
            socket
                .send(reply)
                .await
                .unwrap();
        }
        Content::ShellRequest(ShellRequestMessage::Execute(_)) => todo!(),
        Content::ShellReply(_) => unreachable!("will receive only requests"),
    }
}

async fn handle_stdin(message: ZmqMessage, socket: &mut RouterSocket, iopub: &Mutex<PubSocket>) {
    dbg!(("stdin", message));
    todo!()
}

async fn handle_control(message: ZmqMessage, socket: &mut RouterSocket, iopub: &Mutex<PubSocket>) {
    dbg!(("control", message));
    todo!()
}

async fn handle_heartbeat(message: ZmqMessage, socket: &mut RepSocket) {
    dbg!(("heartbeat", &message));
    socket.send(message).await.unwrap();
}

#[derive(Debug)]
struct Message {
    zmq_identities: Vec<String>,
    header: Header,
    parent_header: Option<Header>,
    metadata: Metadata,
    content: Content,
    buffers: Vec<Bytes>,
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
struct Header {
    msg_id: String,
    session: String,
    username: String,
    date: String,
    msg_type: String, // TODO: make this an enum
    version: String,
}

impl Header {
    fn new(msg_type: impl Into<String>) -> Self {
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
struct Metadata(serde_json::Value);

impl Metadata {
    fn empty() -> Self {
        Metadata(json!({}))
    }
}

#[derive(Debug)]
enum Content {
    ShellRequest(ShellRequestMessage),
    ShellReply(ShellReplyMessage),
}
