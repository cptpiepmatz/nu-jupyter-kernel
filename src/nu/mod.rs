use std::env;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{EngineState, Stack, StateDelta, StateWorkingSet};
use nu_protocol::{ParseError, PipelineData, ShellError, Signals, Span, Value, NU_VARIABLE_ID};
use thiserror::Error;

pub mod commands;
pub mod konst;
pub mod render;

#[allow(clippy::let_and_return)] // i like it here
pub fn initial_engine_state() -> EngineState {
    // TODO: compare with nu_cli::get_engine_state for other contexts
    let engine_state = nu_cmd_lang::create_default_context();
    let engine_state = nu_command::add_shell_command_context(engine_state);
    let engine_state = nu_cmd_extra::add_extra_command_context(engine_state);
    let engine_state = nu_cmd_plugin::add_plugin_command_context(engine_state);
    let engine_state = add_env_context(engine_state);
    let engine_state = configure_engine_state(engine_state);

    #[cfg(feature = "nu-plotters")]
    let engine_state = nu_plotters::add_plotters_command_context(engine_state);

    // this doesn't add the jupyter context, as they need more context

    engine_state
}

fn add_env_context(mut engine_state: EngineState) -> EngineState {
    if let Ok(current_dir) = env::current_dir() {
        engine_state.add_env_var("PWD".to_owned(), Value::String {
            val: current_dir.to_string_lossy().to_string(),
            internal_span: Span::unknown(),
        });
    }

    engine_state
}

fn configure_engine_state(mut engine_state: EngineState) -> EngineState {
    let mut config_dir = env::current_dir().unwrap();
    config_dir.push(".nu");

    engine_state.history_enabled = false;
    engine_state.is_interactive = false;
    engine_state.is_login = false;
    engine_state.set_config_path("config-path", config_dir.join("config.nu"));
    engine_state.set_config_path("env-path", config_dir.join("env.nu"));

    engine_state.generate_nu_constant();

    if let Some(ref v) = engine_state.get_var(NU_VARIABLE_ID).const_val {
        engine_state.plugin_path = v
            .get_data_by_key("plugin-path")
            .and_then(|v| v.as_str().ok().map(PathBuf::from));
    }

    engine_state
}

pub fn add_interrupt_signal(mut engine_state: EngineState) -> (EngineState, Arc<AtomicBool>) {
    let signal = Arc::new(AtomicBool::new(false));
    let signals = Signals::new(signal.clone());
    engine_state.set_signals(signals);
    (engine_state, signal)
}

#[derive(Error)]
pub enum ExecuteError {
    #[error("{error}")]
    Parse {
        #[source]
        error: ParseError,
        /// Delta of the working set.
        ///
        /// By keeping this delta around we later can update another working
        /// set with and with that correctly source the parsing issues.
        delta: StateDelta,
    },

    #[error(transparent)]
    Shell(#[from] ShellError),
}

impl Debug for ExecuteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse { error, delta } => f
                .debug_struct("Parse")
                .field("error", error)
                .field("delta", &format_args!("[StateDelta]"))
                .finish(),
            Self::Shell(arg0) => f.debug_tuple("Shell").field(arg0).finish(),
        }
    }
}

pub fn execute(
    code: &str,
    engine_state: &mut EngineState,
    stack: &mut Stack,
    name: &str,
) -> Result<PipelineData, ExecuteError> {
    let code = code.as_bytes();
    let mut working_set = StateWorkingSet::new(engine_state);
    let block = nu_parser::parse(&mut working_set, Some(name), code, false);

    if let Some(error) = working_set.parse_errors.into_iter().next() {
        return Err(ExecuteError::Parse {
            error,
            delta: working_set.delta,
        });
    }

    engine_state.merge_delta(working_set.delta)?;
    let res =
        nu_engine::eval_block::<WithoutDebug>(engine_state, stack, &block, PipelineData::Empty)?;
    Ok(res)
}

#[cfg(all(test, windows))]
mod tests {

    use std::os::windows::io::OwnedHandle;
    use std::{io, thread};

    use nu_protocol::engine::Stack;

    use super::commands::external::External;
    use super::initial_engine_state;

    #[test]
    fn test_execute() {
        let mut engine_state = initial_engine_state();
        External::apply(&mut engine_state).unwrap();
        let (mut reader, writer) = os_pipe::pipe().unwrap();
        let reader = thread::spawn(move || {
            let mut stdout = io::stdout();
            io::copy(&mut reader, &mut stdout).unwrap();
        });
        let mut stack = Stack::new().stdout_file(OwnedHandle::from(writer).into());
        let code = "plugin add nu_plugin_polars";
        let name = concat!(module_path!(), "::test_execute");
        eprintln!("will execute {code:?} via {name:?}");
        super::execute(code, &mut engine_state, &mut stack, name).unwrap();
        drop(stack);
        reader.join().unwrap();
    }
}
