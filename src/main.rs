use std::env;
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use connection_file::ConnectionFile;
use const_format::formatcp;
use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{self, EngineState, Stack, StateWorkingSet};
use nu_protocol::PipelineData;
use register_kernel::{register_kernel, RegisterLocation};

mod connection_file;
mod register_kernel;
mod execute_nu;

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
    dbg!(connection_file);
    todo!();
}
