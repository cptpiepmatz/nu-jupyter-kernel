// for now
#![allow(dead_code)]
#![allow(unused_variables)]

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, OnceLock};
use std::{os, panic, process, thread};

use clap::{Parser, Subcommand};
use const_format::formatcp;
use handlers::shell::Cell;
use handlers::stream::StreamHandler;
use jupyter::connection_file::ConnectionFile;
use jupyter::messages::iopub;
use jupyter::messages::multipart::Multipart;
use jupyter::register_kernel::{register_kernel, RegisterLocation};
use nu::commands::{add_jupyter_command_context, JupyterCommandContext};
use nu::konst::Konst;
use nu::render::FormatDeclIds;
use nu_protocol::engine::Stack;
use zmq::{Context, Socket, SocketType};

use crate::jupyter::messages::DIGESTER;

mod handlers;
mod jupyter;
mod nu;

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
    let format_decl_ids = FormatDeclIds::find(&engine_state).unwrap();
    nu::commands::hide_incompatible_commands(&mut engine_state).unwrap();
    let konst = Konst::register(&mut engine_state).unwrap();

    let (iopub_tx, iopub_rx) = mpsc::channel();

    let ctx = JupyterCommandContext {
        iopub: iopub_tx.clone(),
        format_decl_ids,
        konst,
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

    let heartbeat_thread = thread::Builder::new()
        .name("heartbeat".to_owned())
        .spawn(move || handlers::heartbeat::handle(sockets.heartbeat))
        .unwrap();
    let iopub_thread = thread::Builder::new()
        .name("iopub".to_owned())
        .spawn(move || handlers::iopub::handle(sockets.iopub, iopub_rx))
        .unwrap();
    let shell_thread = thread::Builder::new()
        .name("shell".to_owned())
        .spawn(move || {
            handlers::shell::handle(
                sockets.shell,
                iopub_tx,
                stdout_handler,
                stderr_handler,
                engine_state,
                stack,
                format_decl_ids,
                konst,
                cell,
            )
        })
        .unwrap();

    // TODO: shutdown threads too

    let _ = heartbeat_thread.join();
    let _ = iopub_thread.join();
    let _ = shell_thread.join();
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
