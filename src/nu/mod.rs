use std::collections::HashMap;
use std::env;
use std::fmt::{Debug, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{EngineState, Stack, StateDelta, StateWorkingSet};
use nu_protocol::{
    CompileError, ParseError, PipelineData, ShellError, Signals, Span, UseAnsiColoring, Value,
    NU_VARIABLE_ID,
};
use thiserror::Error;

pub mod commands;
pub mod konst;
pub mod render;

#[allow(clippy::let_and_return)] // i like it here
pub fn initial_engine_state() -> EngineState {
    // TODO: compare with nu_cli::get_engine_state for other contexts
    let engine_state = nu_cmd_lang::create_default_context();
    let engine_state = configure_engine_state(engine_state);
    let engine_state = add_env_context(engine_state);

    let engine_state = nu_command::add_shell_command_context(engine_state);
    let engine_state = nu_cmd_extra::add_extra_command_context(engine_state);
    let engine_state = nu_cmd_plugin::add_plugin_command_context(engine_state);

    #[cfg(feature = "nu-plotters")]
    let engine_state = nu_plotters::add_plotters_command_context(engine_state);

    // this doesn't add the jupyter context, as they need more context

    engine_state
}

fn add_env_context(mut engine_state: EngineState) -> EngineState {
    let mut env_map = HashMap::new();

    for (key, value) in env::vars() {
        env_map.insert(key, value);
    }

    if let Ok(current_dir) = env::current_dir() {
        env_map.insert("PWD".into(), current_dir.to_string_lossy().into_owned());
    }

    let mut toml = String::new();
    let mut values = Vec::new();
    let mut line_offset = 0;
    for (key, value) in env_map {
        let line = format!("{key} = {value:?}");
        let start = key.len() + " = ".len() + line_offset;
        let end = line_offset + line.len();
        let span = Span::new(start, end);
        line_offset += line.len() + 1;
        writeln!(toml, "{line}").expect("infallible");
        values.push((key, Value::string(value, span)));
    }

    let span_offset = engine_state.next_span_start();
    engine_state.add_file(
        "Host Environment Variables".into(),
        toml.into_bytes().into(),
    );
    for (key, value) in values {
        let span = value.span();
        let span = Span::new(span.start + span_offset, span.end + span_offset);
        engine_state.add_env_var(key, value.with_span(span));
    }

    engine_state
}

fn configure_engine_state(mut engine_state: EngineState) -> EngineState {
    engine_state.history_enabled = false;
    engine_state.is_interactive = false;
    engine_state.is_login = false;

    // if we cannot access the current dir, we probably also cannot access the
    // subdirectories
    if let Ok(mut config_dir) = env::current_dir() {
        config_dir.push(".nu");
        engine_state.set_config_path("config-path", config_dir.join("config.nu"));
        engine_state.set_config_path("env-path", config_dir.join("env.nu"));
    }

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

pub fn execute(
    code: &str,
    engine_state: &mut EngineState,
    stack: &mut Stack,
    name: &str,
) -> Result<PipelineData, ExecuteError> {
    let code = code.as_bytes();
    let mut working_set = StateWorkingSet::new(engine_state);
    let block = nu_parser::parse(&mut working_set, Some(name), code, false);

    // TODO: report parse warnings

    if let Some(error) = working_set.parse_errors.into_iter().next() {
        return Err(ExecuteError::Parse {
            error,
            delta: working_set.delta,
        });
    }

    if let Some(error) = working_set.compile_errors.into_iter().next() {
        return Err(ExecuteError::Compile {
            error,
            delta: working_set.delta,
        });
    }

    engine_state.merge_delta(working_set.delta)?;
    let res =
        nu_engine::eval_block::<WithoutDebug>(engine_state, stack, &block, PipelineData::Empty)?;
    Ok(res)
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

    #[error("{error}")]
    Compile {
        #[source]
        error: CompileError,
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
            Self::Compile { error, delta } => f
                .debug_struct("Compile")
                .field("error", error)
                .field("delta", &format_args!("[StateDelta]"))
                .finish(),
            Self::Shell(arg0) => f.debug_tuple("Shell").field(arg0).finish(),
        }
    }
}

#[derive(Error)]
#[error("{diagnostic}")]
pub struct ReportExecuteError<'s> {
    diagnostic: Box<dyn miette::Diagnostic>,
    working_set: &'s StateWorkingSet<'s>,
}

impl Debug for ReportExecuteError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // This code is stolen from nu_protocol::errors::cli_error::CliError::Debug impl

        let config = self.working_set.get_config();

        let ansi_support = match config.use_ansi_coloring {
            // TODO: design a better auto determination
            UseAnsiColoring::Auto => true,
            UseAnsiColoring::True => true,
            UseAnsiColoring::False => false,
        };

        let error_style = &config.error_style;

        let miette_handler: Box<dyn miette::ReportHandler> = match error_style {
            nu_protocol::ErrorStyle::Plain => Box::new(miette::NarratableReportHandler::new()),
            nu_protocol::ErrorStyle::Fancy => Box::new(
                miette::MietteHandlerOpts::new()
                    // For better support of terminal themes use the ANSI coloring
                    .rgb_colors(miette::RgbColors::Never)
                    // If ansi support is disabled in the config disable the eye-candy
                    .color(ansi_support)
                    .unicode(ansi_support)
                    .terminal_links(ansi_support)
                    .build(),
            ),
        };

        // Ignore error to prevent format! panics. This can happen if span points at
        // some inaccessible location, for example by calling `report_error()`
        // with wrong working set.
        let _ = miette_handler.debug(self, f);

        Ok(())
    }
}

impl<'s> ReportExecuteError<'s> {
    pub fn new(error: ExecuteError, working_set: &'s mut StateWorkingSet<'s>) -> Self {
        let diagnostic = match error {
            ExecuteError::Parse { error, delta } => {
                working_set.delta = delta;
                Box::new(error) as Box<dyn miette::Diagnostic>
            }
            ExecuteError::Compile { error, delta } => {
                working_set.delta = delta;
                Box::new(error) as Box<dyn miette::Diagnostic>
            }
            ExecuteError::Shell(error) => Box::new(error) as Box<dyn miette::Diagnostic>,
        };
        Self {
            diagnostic,
            working_set,
        }
    }

    pub fn code<'a>(&'a self) -> Box<dyn std::fmt::Display + 'a> {
        miette::Diagnostic::code(self)
            .unwrap_or_else(|| Box::new(format_args!("nu-jupyter-kernel::unknown-error")))
    }

    pub fn fmt(&self) -> String {
        format!("Error: {:?}", self)
    }
}

impl<'s> miette::Diagnostic for ReportExecuteError<'s> {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.diagnostic.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.diagnostic.severity()
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.diagnostic.help()
    }

    fn url<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        self.diagnostic.url()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        match self.diagnostic.source_code() {
            None => Some(&self.working_set as &dyn miette::SourceCode),
            Some(source_code) => Some(source_code),
        }
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        self.diagnostic.labels()
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn miette::Diagnostic> + 'a>> {
        self.diagnostic.related()
    }

    fn diagnostic_source(&self) -> Option<&dyn miette::Diagnostic> {
        self.diagnostic.diagnostic_source()
    }
}
