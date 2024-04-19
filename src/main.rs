use std::env;
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use const_format::formatcp;
use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{self, EngineState, Stack, StateWorkingSet};
use nu_protocol::PipelineData;
use register_kernel::{register_kernel, RegisterLocation};

mod connection_file;
mod register_kernel;

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

static INPUT: &str = "{a: 3} | to json";

fn get_engine_state() -> EngineState {
    // TODO: compare with nu_cli::get_engine_state for other contexts
    let engine_state = nu_cmd_lang::create_default_context();
    let engine_state = nu_command::add_shell_command_context(engine_state);
    engine_state
}

fn execute_some_input() {
    let input = INPUT.as_bytes();
    let mut engine_state = get_engine_state();
    let mut working_set = StateWorkingSet::new(&engine_state);
    let block = nu_parser::parse(&mut working_set, None, input, false);

    if let Some(error) = working_set.parse_errors.first() {
        println!("got some error: {error}");
        return;
    }

    engine_state
        .merge_delta(working_set.delta)
        .expect("could not merge delta");
    let mut stack = Stack::new();
    let res = nu_engine::eval_block::<WithoutDebug>(
        &engine_state,
        &mut stack,
        &block,
        PipelineData::empty(),
    );
    dbg!(res);
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
        } => todo!(),
    }
}
