// for now
#![allow(dead_code)]
#![allow(unused_variables)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::{panic, process, thread};

use clap::{Parser, Subcommand};
use const_format::formatcp;
use jupyter::connection_file::ConnectionFile;
use jupyter::messages::iopub::ExecuteResult;
use jupyter::messages::multipart::Multipart;
use jupyter::messages::shell::{ExecuteReply, ExecuteRequest, KernelInfoReply, ShellReplyOk};
use jupyter::register_kernel::{register_kernel, RegisterLocation};
use miette::{Diagnostic, Report};
use mime::Mime;
use nu::commands::external::External;
use nu::render::{FormatDeclIds, PipelineRender};
use nu_protocol::cli_error::CliError;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use parking_lot::Mutex;
use serde_json::json;
use zmq::{Context, Socket, SocketType};

use crate::jupyter::messages::iopub::{IopubBroacast, Status};
use crate::jupyter::messages::shell::{ShellReply, ShellRequest};
use crate::jupyter::messages::{iopub, Header, Message, Metadata, DIGESTER};

mod jupyter;
mod nu;

static_toml::static_toml! {
    const CARGO_TOML = include_toml!("Cargo.toml");
}

static EXECUTION_COUNTER: AtomicUsize = AtomicUsize::new(1);
static CELL_EVAL_COUNTER: AtomicUsize = AtomicUsize::new(1);
static RENDER_FILTER: Mutex<Option<Mime>> = Mutex::new(Option::None);

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

    let mut engine_state = nu::initial_engine_state();
    let to_decl_ids = FormatDeclIds::find(&engine_state).unwrap();
    nu::commands::hide_incompatible_commands(&mut engine_state).unwrap();

    // let mut working_set = StateWorkingSet::new(&engine_state);
    // working_set.add_decl(Box::new(External));
    // engine_state.merge_delta(working_set.render()).unwrap();

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
        .spawn(move || handle_shell(sockets.shell, iopub_tx, engine_state, to_decl_ids))
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
    format_decl_ids: FormatDeclIds,
) {
    let mut stack = Stack::new();
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

        match &message.content {
            ShellRequest::Execute(request) => handle_execute_request(
                &socket,
                &message,
                &iopub,
                request,
                &mut engine_state,
                &mut stack,
                format_decl_ids,
            ),
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

fn handle_execute_request(
    socket: &Socket,
    message: &Message<ShellRequest>,
    iopub: &mpsc::Sender<Multipart>,
    request: &ExecuteRequest,
    engine_state: &mut EngineState,
    stack: &mut Stack,
    format_decl_ids: FormatDeclIds,
) {
    let ExecuteRequest {
        code,
        silent,
        store_history,
        user_expressions,
        allow_stdin,
        stop_on_error,
    } = request;
    let msg_type = ShellReply::msg_type(&message.header.msg_type).unwrap();
    External::apply(engine_state).unwrap();

    let cell_name = format!(
        "cell-{}-{}",
        EXECUTION_COUNTER.load(Ordering::SeqCst),
        CELL_EVAL_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let pipeline_data = match nu::execute(&code, engine_state, stack, &cell_name) {
        Ok(data) => data,
        Err(e) => {
            let mut working_set = StateWorkingSet::new(engine_state);
            let report = match e {
                nu::ExecuteError::Parse { ref error, delta } => {
                    working_set.delta = delta;
                    error as &(dyn miette::Diagnostic + Send + Sync + 'static)
                }
                nu::ExecuteError::Shell(ref error) => {
                    error as &(dyn miette::Diagnostic + Send + Sync + 'static)
                }
            };

            let name = report
                .code()
                .unwrap_or_else(|| Box::new(format_args!("nu-jupyter-kernel::unknown-error")));
            let value = format!("Error: {:?}", CliError(report, &working_set));
            // TODO: for traceback use error source
            let traceback = vec![];

            // we send display data to have control over the rendering of the output
            let broadcast = IopubBroacast::DisplayData(iopub::DisplayData {
                data: HashMap::from([(mime::TEXT_PLAIN.to_string(), value.clone())]),
                metadata: HashMap::new(),
                transient: HashMap::new(),
            });
            let broadcast = Message {
                zmq_identities: message.zmq_identities.clone(),
                header: Header::new(broadcast.msg_type()),
                parent_header: Some(message.header.clone()),
                metadata: Metadata::empty(),
                content: broadcast,
                buffers: vec![],
            };
            iopub.send(broadcast.into_multipart().unwrap()).unwrap();

            let reply = ShellReply::Error {
                name: name.to_string(),
                value,
                traceback,
            };
            let reply = Message {
                zmq_identities: message.zmq_identities.clone(),
                header: Header::new(msg_type),
                parent_header: Some(message.header.clone()),
                metadata: Metadata::empty(),
                content: reply,
                buffers: vec![],
            };
            reply.into_multipart().unwrap().send(socket).unwrap();
            return;
        }
    };

    let execution_count = EXECUTION_COUNTER.fetch_add(1, Ordering::SeqCst);
    // reset eval counter to keep last digit of cell names as tries for that cell
    CELL_EVAL_COUNTER.store(1, Ordering::SeqCst);

    if !pipeline_data.is_nothing() {
        let mut render_filter = RENDER_FILTER.lock();
        let render = PipelineRender::render(
            pipeline_data,
            engine_state,
            stack,
            format_decl_ids,
            render_filter.take(),
        );

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
    }

    let reply = ExecuteReply {
        execution_count,
        user_expressions: json!({}),
    };
    let reply = ShellReply::Ok(ShellReplyOk::Execute(reply));
    let reply = Message {
        zmq_identities: message.zmq_identities.clone(),
        header: Header::new(msg_type),
        parent_header: Some(message.header.clone()),
        metadata: Metadata::empty(),
        content: reply,
        buffers: vec![],
    };
    reply.into_multipart().unwrap().send(&socket).unwrap();
}

// no heartbeat nor iopub as they are handled differently
#[derive(Debug, Clone, Copy)]
enum Channel {
    Shell,
    Iopub,
    Stdin,
    Control,
}

fn set_avalanche_panic_hook() {
    let old_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        old_hook(panic_info);
        process::exit(1);
    }));
}
