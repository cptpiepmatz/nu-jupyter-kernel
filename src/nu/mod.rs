use std::env;
use std::fmt::Display;

use miette::Diagnostic;
use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{ParseError, PipelineData, ShellError, Span, Value};
use thiserror::Error;

pub mod render;

pub fn initial_engine_state() -> EngineState {
    // TODO: compare with nu_cli::get_engine_state for other contexts
    let engine_state = nu_cmd_lang::create_default_context();
    let engine_state = nu_command::add_shell_command_context(engine_state);
    let engine_state = nu_cmd_extra::add_extra_command_context(engine_state);
    let engine_state = add_env_context(engine_state);

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

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    Shell(#[from] ShellError),
}

impl Diagnostic for ExecuteError {
    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        match self {
            Self::Parse(e) => e.code(),
            Self::Shell(e) => e.code(),
        }
    }
}

pub fn execute(
    code: &str,
    engine_state: &mut EngineState,
    stack: &mut Stack,
) -> Result<PipelineData, ExecuteError> {
    let code = code.as_bytes();
    let mut working_set = StateWorkingSet::new(engine_state);
    // TODO: use for fname the history counter
    let block = nu_parser::parse(&mut working_set, None, code, false);

    if let Some(error) = working_set.parse_errors.into_iter().next() {
        return Err(error.into());
    }

    engine_state.merge_delta(working_set.delta)?;
    let res =
        nu_engine::eval_block::<WithoutDebug>(engine_state, stack, &block, PipelineData::Empty)?;
    Ok(res)
}
