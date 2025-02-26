// for now
#![allow(dead_code)]
#![allow(unused_variables)]

use std::path::{Path, PathBuf};
use std::{panic, process};

use clap::{Parser, Subcommand};
use const_format::formatcp;
use handlers::shell::Cell;
use handlers::stream::StreamHandler;
use jupyter::connection_file::ConnectionFile;
use jupyter::messages::iopub;
use jupyter::register_kernel::{register_kernel, RegisterLocation};
use nu::commands::{add_jupyter_command_context, JupyterCommandContext};
use nu::konst::Konst;
use nu::render::FormatDeclIds;
use nu_protocol::engine::Stack;
use tokio::sync::{broadcast, mpsc};
use zeromq::{PubSocket, RepSocket, RouterSocket, Socket, ZmqResult};

use crate::jupyter::messages::DIGESTER;

mod error;
mod handlers;
mod jupyter;
mod nu;
mod util;

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

type ShellSocket = RouterSocket;
type IopubSocket = PubSocket;
type StdinSocket = RouterSocket;
type ControlSocket = RouterSocket;
type HeartbeatSocket = RepSocket;

struct Sockets {
    pub shell: ShellSocket,
    pub iopub: IopubSocket,
    pub stdin: StdinSocket,
    pub control: ControlSocket,
    pub heartbeat: HeartbeatSocket,
}

impl Sockets {
    async fn start(connection_file: &ConnectionFile) -> ZmqResult<Self> {
        let endpoint = |port| {
            format!(
                "{}://{}:{}",
                connection_file.transport, connection_file.ip, port
            )
        };

        let mut shell = ShellSocket::new();
        shell.bind(&endpoint(&connection_file.shell_port)).await?;

        let mut iopub = IopubSocket::new();
        iopub.bind(&endpoint(&connection_file.iopub_port)).await?;

        let mut stdin = StdinSocket::new();
        stdin.bind(&endpoint(&connection_file.stdin_port)).await?;

        let mut control = ControlSocket::new();
        control
            .bind(&endpoint(&connection_file.control_port))
            .await?;

        let mut heartbeat = HeartbeatSocket::new();
        heartbeat
            .bind(&endpoint(&connection_file.heartbeat_port))
            .await?;

        Ok(Sockets {
            shell,
            iopub,
            stdin,
            control,
            heartbeat,
        })
    }
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let args = Cli::parse();
    match args.command {
        Command::Register { user, system } => {
            let location = match (user, system) {
                (true, true) => unreachable!("handled by clap"),
                (false, true) => RegisterLocation::System,
                (true, false) => RegisterLocation::User,
                (false, false) => RegisterLocation::User, // default case
            };
            let path = register_kernel(location)?;
            println!("Registered kernel to {}", path.display());
        }
        Command::Start {
            connection_file_path,
        } => start_kernel(connection_file_path).await,
    }
    Ok(())
}

async fn start_kernel(connection_file_path: impl AsRef<Path>) {
    set_avalanche_panic_hook();

    let connection_file = ConnectionFile::from_path(connection_file_path).unwrap();
    let sockets = Sockets::start(&connection_file).await.unwrap();
    DIGESTER.key_init(&connection_file.key).unwrap();

    let mut engine_state = nu::initial_engine_state();
    let format_decl_ids = FormatDeclIds::find(&engine_state).unwrap();
    let spans = nu::module::create_nuju_module(&mut engine_state);
    nu::commands::hide_incompatible_commands(&mut engine_state).unwrap();
    let konst = Konst::register(&mut engine_state).unwrap();
    let (engine_state, interrupt_signal) = nu::add_interrupt_signal(engine_state);

    let (iopub_tx, iopub_rx) = mpsc::channel(1);
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let ctx = JupyterCommandContext {
        iopub: iopub_tx.clone(),
        format_decl_ids,
        konst,
        spans: spans.clone(),
    };
    let engine_state = add_jupyter_command_context(engine_state, ctx);

    let (stdout_handler, stdout_file) =
        StreamHandler::start(iopub::StreamName::Stdout, iopub_tx.clone()).unwrap();
    let (stderr_handler, stderr_file) =
        StreamHandler::start(iopub::StreamName::Stderr, iopub_tx.clone()).unwrap();
    let stack = Stack::new()
        .stdout_file(stdout_file)
        .stderr_file(stderr_file);

    let cell = Cell::new();

    let heartbeat_task = tokio::spawn(handlers::heartbeat::handle(
        sockets.heartbeat,
        shutdown_rx.resubscribe(),
    ));

    let iopub_task = tokio::spawn(handlers::iopub::handle(
        sockets.iopub,
        shutdown_rx.resubscribe(),
        iopub_rx,
    ));

    let shell_ctx = handlers::shell::HandlerContext {
        socket: sockets.shell,
        iopub: iopub_tx,
        stdout_handler,
        stderr_handler,
        engine_state,
        format_decl_ids,
        konst,
        spans,
        stack,
        cell,
    };
    let shell_task = tokio::spawn(handlers::shell::handle(
        shell_ctx,
        shutdown_rx.resubscribe(),
    ));

    let control_task = tokio::spawn(handlers::control::handle(
        sockets.control,
        shutdown_tx,
        interrupt_signal,
    ));

    heartbeat_task.await.unwrap();
    iopub_task.await.unwrap();
    shell_task.await.unwrap();
    control_task.await.unwrap();
}

// no heartbeat nor iopub as they are handled differently
#[derive(Debug, Clone, Copy)]
enum Channel {
    Shell,
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
