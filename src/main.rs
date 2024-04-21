// for now
#![allow(dead_code)]
#![allow(unused_variables)]

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use const_format::formatcp;
use hmac::Mac;
use jupyter::connection_file::ConnectionFile;
use jupyter::messages::shell::{
    KernelInfoReply, ShellReplyMessage, ShellReplyOk, ShellRequestMessage,
};
use jupyter::register_kernel::{register_kernel, RegisterLocation};
use parking_lot::Mutex;
use tokio::select;
use zeromq::{PubSocket, RepSocket, RouterSocket, Socket, SocketRecv, SocketSend, ZmqMessage};

use crate::jupyter::messages::{Content, Header, Message, Metadata, DIGESTER, KERNEL_SESSION};

mod execute_nu;
mod jupyter;

static_toml::static_toml! {
    const CARGO_TOML = include_toml!("Cargo.toml");
}

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
            socket.send(reply).await.unwrap();
        }
        Content::ShellRequest(ShellRequestMessage::Execute(_)) => todo!(),
        Content::ShellReply(_) => unreachable!("will receive only requests"),
    }
}

async fn handle_stdin(message: ZmqMessage, socket: &mut RouterSocket, iopub: &Mutex<PubSocket>) {
    dbg!(("stdin", message));
    todo!("handle stdin")
}

async fn handle_control(message: ZmqMessage, socket: &mut RouterSocket, iopub: &Mutex<PubSocket>) {
    dbg!(("control", message));
    todo!("handle control")
}

async fn handle_heartbeat(message: ZmqMessage, socket: &mut RepSocket) {
    dbg!(("heartbeat", &message));
    socket.send(message).await.unwrap();
}
