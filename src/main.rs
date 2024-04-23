// for now
#![allow(dead_code)]
#![allow(unused_variables)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, OnceLock};
use std::{panic, process, thread};

use clap::{Parser, Subcommand};
use const_format::formatcp;
use jupyter::connection_file::{self, ConnectionFile};
use jupyter::messages::iopub::ExecuteResult;
use jupyter::messages::multipart::Multipart;
use jupyter::messages::shell::{
    ExecuteReply, ExecuteRequest, IsCompleteReply, KernelInfoReply, ShellReplyOk,
};
use jupyter::register_kernel::{register_kernel, RegisterLocation};
use nu::{PipelineRender, ToDeclIds};
use nu_protocol::engine::{EngineState, Stack};
use serde_json::json;
use zmq::{Context, Socket, SocketType};

use crate::jupyter::messages::iopub::{IopubBroacast, Status};
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
    #[command(alias = "install")]
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

struct Sockets {
    pub shell: Socket,
    pub iopub: Socket,
    pub stdin: Socket,
    pub control: Socket,
    pub heartbeat: Socket,
}

impl Sockets {
    fn start(connection_file: &ConnectionFile) -> zmq::Result<Self> {
        let endpoint = |port| {
            format!(
                "{}://{}:{}",
                connection_file.transport, connection_file.ip, port
            )
        };

        let shell = Context::new().socket(SocketType::ROUTER)?;
        shell.bind(&endpoint(&connection_file.shell_port))?;

        let iopub = Context::new().socket(SocketType::PUB)?;
        iopub.bind(&endpoint(&connection_file.iopub_port))?;

        let stdin = Context::new().socket(SocketType::ROUTER)?;
        stdin.bind(&endpoint(&connection_file.stdin_port))?;

        let control = Context::new().socket(SocketType::ROUTER)?;
        control.bind(&endpoint(&connection_file.control_port))?;

        let heartbeat = Context::new().socket(SocketType::REP)?;
        heartbeat.bind(&endpoint(&connection_file.heartbeat_port))?;

        Ok(Sockets {
            shell,
            iopub,
            stdin,
            control,
            heartbeat,
        })
    }
}
fn main() {
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
        } => start_kernel(connection_file_path),
    }
}

fn start_kernel(connection_file_path: impl AsRef<Path>) {
    set_avalanche_panic_hook();

    let connection_file = ConnectionFile::from_path(connection_file_path).unwrap();
    let sockets = Sockets::start(&connection_file).unwrap();
    DIGESTER.key_init(&connection_file.key).unwrap();

    let engine_state = nu::initial_engine_state();
    let stack = Stack::new();
    let to_decl_ids = ToDeclIds::find(&engine_state).unwrap();

    let (iopub_tx, iopub_rx) = mpsc::channel();

    let heartbeat_thread = thread::Builder::new()
        .name("heartbeat".to_owned())
        .spawn(move || handle_heartbeat(sockets.heartbeat))
        .unwrap();
    let iopub_thread = thread::Builder::new()
        .name("iopub".to_owned())
        .spawn(move || handle_iopub(sockets.iopub, iopub_rx))
        .unwrap();
    let shell_thread = thread::Builder::new()
        .name("shell".to_owned())
        .spawn(move || handle_shell(sockets.shell, iopub_tx, engine_state, stack, to_decl_ids))
        .unwrap();

    // TODO: shutdown threads too

    let _ = heartbeat_thread.join();
    let _ = iopub_thread.join();
    let _ = shell_thread.join();
}

fn handle_shell(
    socket: Socket,
    iopub: mpsc::Sender<Multipart>,
    mut engine_state: EngineState,
    mut stack: Stack,
    to_decl_ids: ToDeclIds,
) {
    loop {
        let message = match Message::recv(&socket) {
            Err(_) => {
                eprintln!("could not recv message");
                continue;
            }
            Ok(message) => message,
        };

        iopub
            .send(
                Status::Busy
                    .into_message(message.header.clone())
                    .into_multipart()
                    .unwrap(),
            )
            .unwrap();

        match message.content {
            ShellRequest::Execute(ExecuteRequest {
                code,
                silent,
                store_history,
                user_expressions,
                allow_stdin,
                stop_on_error,
            }) => {
                let pipeline_data = nu::execute(&code, &mut engine_state, &mut stack).unwrap();
                let execution_count = EXECUTION_COUNTER.fetch_add(1, Ordering::SeqCst);
                let render =
                    PipelineRender::render(pipeline_data, &engine_state, &mut stack, to_decl_ids);

                let execute_result = ExecuteResult {
                    execution_count,
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
                let broadcast = IopubBroacast::from(execute_result);
                let broadcast = Message {
                    zmq_identities: message.zmq_identities.clone(),
                    header: Header::new(broadcast.msg_type()),
                    parent_header: Some(message.header.clone()),
                    metadata: Metadata::empty(),
                    content: broadcast,
                    buffers: vec![],
                };
                iopub.send(broadcast.into_multipart().unwrap()).unwrap();

                let reply = ExecuteReply {
                    execution_count,
                    user_expressions: json!({}),
                };
                let reply = ShellReply::Ok(ShellReplyOk::Execute(reply));
                let msg_type = ShellReply::msg_type(&message.header.msg_type).unwrap();
                let reply = Message {
                    zmq_identities: message.zmq_identities,
                    header: Header::new(msg_type),
                    parent_header: Some(message.header.clone()),
                    metadata: Metadata::empty(),
                    content: reply,
                    buffers: vec![],
                };
                reply.into_multipart().unwrap().send(&socket).unwrap();
            }
            ShellRequest::IsComplete(_) => todo!(),
            ShellRequest::KernelInfo => {
                let kernel_info = KernelInfoReply::get();
                let reply = ShellReply::Ok(ShellReplyOk::KernelInfo(kernel_info));
                let msg_type = ShellReply::msg_type(&message.header.msg_type).unwrap();
                let reply = Message {
                    zmq_identities: message.zmq_identities,
                    header: Header::new(msg_type),
                    parent_header: Some(message.header.clone()),
                    metadata: Metadata::empty(),
                    content: reply,
                    buffers: vec![],
                };
                reply.into_multipart().unwrap().send(&socket).unwrap();
            }
        }

        iopub
            .send(
                Status::Idle
                    .into_message(message.header)
                    .into_multipart()
                    .unwrap(),
            )
            .unwrap();
    }
}

fn handle_iopub(socket: Socket, iopub_rx: mpsc::Receiver<Multipart>) {
    loop {
        let multipart = iopub_rx.recv().unwrap();
        multipart.send(&socket).unwrap();
    }
}

fn handle_heartbeat(socket: Socket) {
    loop {
        let msg = socket.recv_multipart(0).unwrap();
        socket.send_multipart(msg, 0).unwrap();
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

// async fn handle_shell(
//     message: ZmqMessage,
//     reply_tx: tokio::sync::oneshot::Sender<ZmqMessage>,
//     iopub: Arc<Mutex<PubSocket>>,
//     engine_state: Arc<Mutex<EngineState>>,
//     stack: Arc<Mutex<Stack>>,
//     to_decl_ids: ToDeclIds,
// ) {
//     let channel = Channel::Shell;
//     let message = Message::parse(message, channel).unwrap();
//     let session = KERNEL_SESSION.get();
//     let mut iopub = iopub.lock().await;

//     match message.content {
//         jupyter::messages::IncomingContent::Shell(ShellRequest::KernelInfo)
// => {             let reply = KernelInfoReply::get();
//             let reply = Message {
//                 zmq_identities: message.zmq_identities,
//                 header: Header::new("kernel_info_reply"),
//                 parent_header: Some(message.header),
//                 metadata: Metadata::empty(),
//                 content:
// OutgoingContent::Shell(ShellReply::Ok(ShellReplyOk::KernelInfo(
// KernelInfoReply::get(),                 ))),
//                 buffers: vec![],
//             };
//             let reply = reply.serialize(channel).unwrap();
//             reply_tx.send(reply).unwrap();

//             let idle_message =
// jupyter::messages::status(iopub::Status::Idle).unwrap();
// iopub.send(idle_message).await.unwrap();         }
//         IncomingContent::Shell(ShellRequest::IsComplete(_)) => {
//             // TODO: not always return unknown
//             let reply =
// ShellReply::Ok(ShellReplyOk::IsComplete(IsCompleteReply::Unknown));
//             let reply = Message {
//                 zmq_identities: message.zmq_identities,
//                 header:
// Header::new(ShellReply::msg_type(&message.header.msg_type).unwrap()),
//                 parent_header: Some(message.header),
//                 metadata: Metadata::empty(),
//                 content: OutgoingContent::Shell(reply),
//                 buffers: vec![],
//             };
//             let reply = reply.serialize(channel).unwrap();
//             reply_tx.send(reply).unwrap();
//         }
//         IncomingContent::Shell(ShellRequest::Execute(ExecuteRequest {
//             code,
//             silent,
//             store_history,
//             user_expressions,
//             allow_stdin,
//             stop_on_error,
//         })) => {
//             let mut engine_state = engine_state.lock().await;
//             let mut stack = stack.lock().await;

//             let busy =
// jupyter::messages::status(iopub::Status::Busy).unwrap();
// iopub.send(busy).await.unwrap();

//             match nu::execute(&code, &mut engine_state, &mut stack) {
//                 Err(_) => todo!(),
//                 Ok(pipeline_data) => {
//                     let execution_count = EXECUTION_COUNTER.fetch_add(1,
// Ordering::SeqCst);                     let render = PipelineRender::render(
//                         pipeline_data,
//                         &engine_state,
//                         &mut stack,
//                         to_decl_ids,
//                     );
//                     let execute_result = ExecuteResult {
//                         execution_count, // TODO: do real number here
//                         data: render
//                             .data
//                             .into_iter()
//                             .map(|(k, v)| (k.to_string(), v))
//                             .collect(),
//                         metadata: render
//                             .metadata
//                             .into_iter()
//                             .map(|(k, v)| (k.to_string(), v))
//                             .collect(),
//                     };
//                     let broadcast =
// IopubBroacast::ExecuteResult(execute_result);                     let
// broadcast = Message {                         zmq_identities:
// message.zmq_identities.clone(),                         header:
// Header::new(broadcast.msg_type()),                         parent_header:
// Some(message.header.clone()),                         metadata:
// Metadata::empty(),                         content:
// OutgoingContent::Iopub(broadcast),                         buffers: vec![],
//                     };
//                     iopub
//                         .send(broadcast.serialize(Channel::Iopub).unwrap())
//                         .await
//                         .unwrap();

//                     let reply = ExecuteReply {
//                         execution_count,
//                         user_expressions: json!({}),
//                     };
//                     let reply = ShellReply::Ok(ShellReplyOk::Execute(reply));
//                     let msg_type =
// ShellReply::msg_type(&message.header.msg_type).unwrap();
// let reply = Message {                         zmq_identities:
// message.zmq_identities,                         header:
// Header::new(msg_type),                         parent_header:
// Some(message.header),                         metadata: Metadata::empty(),
//                         content: OutgoingContent::Shell(reply),
//                         buffers: vec![],
//                     };
//                     reply_tx
//                         .send(reply.serialize(Channel::Shell).unwrap())
//                         .unwrap();
//                 }
//             }

//             let idle =
// jupyter::messages::status(iopub::Status::Idle).unwrap();
// iopub.send(idle).await.unwrap();         }
//     }
// }

// async fn handle_stdin(message: ZmqMessage, socket: &mut RouterSocket) {
//     dbg!(("stdin", message));
//     todo!("handle stdin")
// }

// async fn handle_control(message: ZmqMessage, socket: &mut RouterSocket) {
//     dbg!(("control", message));
//     todo!("handle control")
// }

fn set_avalanche_panic_hook() {
    let old_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        old_hook(panic_info);
        process::exit(1);
    }));
}
