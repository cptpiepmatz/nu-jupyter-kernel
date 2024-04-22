// for now
#![allow(dead_code)]
#![allow(unused_variables)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use clap::{Parser, Subcommand};
use const_format::formatcp;
use jupyter::connection_file::ConnectionFile;
use jupyter::messages::iopub::ExecuteResult;
use jupyter::messages::shell::{
    ExecuteReply, ExecuteRequest, IsCompleteReply, KernelInfoReply, ShellReplyOk,
};
use jupyter::register_kernel::{register_kernel, RegisterLocation};
use nu::{PipelineRender, ToDeclIds};
use nu_protocol::engine::{EngineState, Stack};
use serde_json::json;
use tokio::sync::Mutex;
use zeromq::{PubSocket, RepSocket, RouterSocket, Socket, SocketRecv, SocketSend, ZmqMessage};

use crate::jupyter::messages::iopub::IopubBroacast;
use crate::jupyter::messages::shell::{ShellReply, ShellRequest};
use crate::jupyter::messages::{
    iopub, Header, IncomingContent, Message, Metadata, OutgoingContent, DIGESTER, KERNEL_SESSION,
};

mod jupyter;
mod nu;

static_toml::static_toml! {
    const CARGO_TOML = include_toml!("Cargo.toml");
}

static EXECUTION_COUNTER: AtomicUsize = AtomicUsize::new(1);

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
    let endpoint = |port| format!("{}://127.0.0.1:{}", connection_file.transport, port);

    DIGESTER.key_init(&connection_file.key).unwrap();
    let iopub_endpoint = endpoint(connection_file.iopub_port);
    let mut iopub_socket = PubSocket::new();
    let iopub_endpoint = iopub_socket.bind(&iopub_endpoint).await.unwrap();

    // send out the starting message as soon as possible
    let starting_message = jupyter::messages::status(iopub::Status::Starting).unwrap();
    iopub_socket.send(starting_message).await.unwrap();

    let shell_endpoint = endpoint(connection_file.shell_port);
    let mut shell_socket = RouterSocket::new();
    let shell_endpoint = shell_socket.bind(&shell_endpoint).await.unwrap();

    let stdin_endpoint = endpoint(connection_file.stdin_port);
    let mut stdin_socket = RouterSocket::new();
    let stdin_endpoint = stdin_socket.bind(&stdin_endpoint).await.unwrap();

    let control_endpoint = endpoint(connection_file.control_port);
    let mut control_socket = RouterSocket::new();
    let control_endpoint = control_socket.bind(&control_endpoint).await.unwrap();

    let heartbeat_endpoint = endpoint(connection_file.heartbeat_port);
    let mut heartbeat_socket = RepSocket::new();
    let heartbeat_endpoint = heartbeat_socket.bind(&heartbeat_endpoint).await.unwrap();

    let engine_state = nu::initial_engine_state();
    let to_decl_ids = ToDeclIds::find(&engine_state).unwrap();
    let engine_state = Arc::new(Mutex::new(engine_state));
    let stack = Arc::new(Mutex::new(Stack::new()));

    let (ch_tx, mut ch_rx) = tokio::sync::mpsc::channel(10);

    let shell_tx = ch_tx.clone();
    let shell_actor = tokio::spawn(async move {
        let mut socket = shell_socket;
        loop {
            let msg = match socket.recv().await {
                Err(_) => todo!("handle error receiving from a socket"),
                Ok(msg) => msg,
            };
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            shell_tx
                .send((Channel::Shell, msg, reply_tx))
                .await
                .unwrap();
            let reply = match reply_rx.await {
                Err(_) => continue,
                Ok(reply) => reply,
            };
            if socket.send(reply).await.is_err() {
                todo!("handle error sending to socket");
            }
        }
    });

    // TODO: add other actors for other channels

    let heartbeat_handler = tokio::spawn(async move {
        let mut socket = heartbeat_socket;
        loop {
            let heartbeat = socket.recv().await.unwrap();
            socket.send(heartbeat).await.unwrap();
        }
    });

    let idle_message = jupyter::messages::status(iopub::Status::Idle).unwrap();
    iopub_socket.send(idle_message).await.unwrap();
    let iopub_socket = Arc::new(Mutex::new(iopub_socket));

    while let Some((channel, zmq_message, reply_tx)) = ch_rx.recv().await {
        match channel {
            Channel::Shell => tokio::spawn(handle_shell(
                zmq_message,
                reply_tx,
                iopub_socket.clone(),
                engine_state.clone(),
                stack.clone(),
                to_decl_ids,
            )),
            Channel::Iopub => unreachable!("no receiving on iopub"),
            Channel::Stdin => todo!(),
            Channel::Control => todo!(),
        };
    }
}

// no heartbeat nor iopub as they are handled differently
#[derive(Debug, Clone, Copy)]
enum Channel {
    Shell,
    Iopub,
    Stdin,
    Control,
}

async fn handle_shell(
    message: ZmqMessage,
    reply_tx: tokio::sync::oneshot::Sender<ZmqMessage>,
    iopub: Arc<Mutex<PubSocket>>,
    engine_state: Arc<Mutex<EngineState>>,
    stack: Arc<Mutex<Stack>>,
    to_decl_ids: ToDeclIds,
) {
    let channel = Channel::Shell;
    let message = Message::parse(message, channel).unwrap();
    let session = KERNEL_SESSION.get();
    match message.content {
        jupyter::messages::IncomingContent::Shell(ShellRequest::KernelInfo) => {
            let reply = KernelInfoReply::get();
            let reply = Message {
                zmq_identities: message.zmq_identities,
                header: Header::new("kernel_info_reply"),
                parent_header: Some(message.header),
                metadata: Metadata::empty(),
                content: OutgoingContent::Shell(ShellReply::Ok(ShellReplyOk::KernelInfo(
                    KernelInfoReply::get(),
                ))),
                buffers: vec![],
            };
            let reply = reply.serialize(channel).unwrap();
            reply_tx.send(reply).unwrap();
        }
        IncomingContent::Shell(ShellRequest::IsComplete(_)) => {
            // TODO: not always return unknown
            let reply = ShellReply::Ok(ShellReplyOk::IsComplete(IsCompleteReply::Unknown));
            let reply = Message {
                zmq_identities: message.zmq_identities,
                header: Header::new(ShellReply::msg_type(&message.header.msg_type).unwrap()),
                parent_header: Some(message.header),
                metadata: Metadata::empty(),
                content: OutgoingContent::Shell(reply),
                buffers: vec![],
            };
            let reply = reply.serialize(channel).unwrap();
            reply_tx.send(reply).unwrap();
        }
        IncomingContent::Shell(ShellRequest::Execute(ExecuteRequest {
            code,
            silent,
            store_history,
            user_expressions,
            allow_stdin,
            stop_on_error,
        })) => {
            let mut engine_state = engine_state.lock().await;
            let mut stack = stack.lock().await;
            let mut iopub = iopub.lock().await;

            let busy = jupyter::messages::status(iopub::Status::Busy).unwrap();
            iopub.send(busy).await.unwrap();

            match nu::execute(&code, &mut engine_state, &mut stack) {
                Err(_) => todo!(),
                Ok(pipeline_data) => {
                    let execution_count = EXECUTION_COUNTER.fetch_add(1, Ordering::SeqCst);
                    let render = PipelineRender::render(
                        pipeline_data,
                        &engine_state,
                        &mut stack,
                        to_decl_ids,
                    );
                    let execute_result = ExecuteResult {
                        execution_count, // TODO: do real number here
                        data: render
                            .data
                            .into_iter()
                            .map(|(k, v)| (k.to_string(), v))
                            .collect(),
                        metadata: render
                            .metadata
                            .into_iter()
                            .map(|(k, v)| (k.to_string(), v))
                            .collect(),
                    };
                    let broadcast = IopubBroacast::ExecuteResult(execute_result);
                    let broadcast = Message {
                        zmq_identities: message.zmq_identities.clone(),
                        header: Header::new(broadcast.msg_type()),
                        parent_header: Some(message.header.clone()),
                        metadata: Metadata::empty(),
                        content: OutgoingContent::Iopub(broadcast),
                        buffers: vec![],
                    };
                    iopub
                        .send(broadcast.serialize(Channel::Iopub).unwrap())
                        .await
                        .unwrap();

                    let reply = ExecuteReply {
                        execution_count,
                        user_expressions: json!({}),
                    };
                    let reply = ShellReply::Ok(ShellReplyOk::Execute(reply));
                    let msg_type = ShellReply::msg_type(&message.header.msg_type).unwrap();
                    let reply = Message {
                        zmq_identities: message.zmq_identities,
                        header: Header::new(msg_type),
                        parent_header: Some(message.header),
                        metadata: Metadata::empty(),
                        content: OutgoingContent::Shell(reply),
                        buffers: vec![],
                    };
                    reply_tx
                        .send(reply.serialize(Channel::Shell).unwrap())
                        .unwrap();
                }
            }
            // responde to request here too

            let idle = jupyter::messages::status(iopub::Status::Idle).unwrap();
            iopub.send(idle).await.unwrap();
        }
    }
}

async fn handle_stdin(message: ZmqMessage, socket: &mut RouterSocket) {
    dbg!(("stdin", message));
    todo!("handle stdin")
}

async fn handle_control(message: ZmqMessage, socket: &mut RouterSocket) {
    dbg!(("control", message));
    todo!("handle control")
}
