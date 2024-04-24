use std::collections::HashMap;
use std::env;
use std::fmt::Display;

use miette::Diagnostic;
use mime::Mime;
use nu_command::{ToCsv, ToJson, ToMd, ToText};
use nu_protocol::ast::Call;
use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{Command, EngineState, Stack, StateWorkingSet};
use nu_protocol::{ParseError, PipelineData, ShellError, Span, Value};
use thiserror::Error;

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

#[derive(Debug, Clone, Copy)]
pub struct ToDeclIds {
    pub to_text: usize,
    pub to_csv: usize,
    pub to_json: usize,
    pub to_html: usize,
    pub to_md: usize,
}

impl ToDeclIds {
    pub fn find(engine_state: &EngineState) -> Result<ToDeclIds, ()> {
        let mut to_text = None;
        let mut to_csv = None;
        let mut to_json = None;
        let mut to_html = None;
        let mut to_md = None;

        for (str_bytes, decl_id) in engine_state.get_decls_sorted(false) {
            let s = String::from_utf8(str_bytes).unwrap();
            match s.as_str() {
                "to text" => to_text = Some(decl_id),
                "to csv" => to_csv = Some(decl_id),
                "to json" => to_json = Some(decl_id),
                "to html" => to_html = Some(decl_id),
                "to md" => to_md = Some(decl_id),
                _ => (),
            }
        }

        if let (Some(to_text), Some(to_csv), Some(to_json), Some(to_html), Some(to_md)) =
            (to_text, to_csv, to_json, to_html, to_md)
        {
            return Ok(ToDeclIds {
                to_text,
                to_csv,
                to_json,
                to_html,
                to_md,
            });
        }

        todo!()
    }
}

#[derive(Debug)]
pub struct PipelineRender {
    pub data: HashMap<Mime, String>,
    pub metadata: HashMap<Mime, String>,
}

impl PipelineRender {
    fn render_via_cmd(
        value: &Value,
        to_cmd: impl Command,
        decl_id: usize,
        engine_state: &EngineState,
        stack: &mut Stack,
        data: &mut HashMap<Mime, String>,
        mime: &str,
    ) -> bool {
        let ty = value.get_type();
        let may = to_cmd
            .signature()
            .input_output_types
            .iter()
            .map(|(input, _)| input)
            .any(|input| ty.is_subtype(input));
        match may {
            true => Self::render_via_call(value.clone(), decl_id, engine_state, stack, data, mime),
            false => false,
        }
    }

    fn render_via_call(
        value: Value,
        decl_id: usize,
        engine_state: &EngineState,
        stack: &mut Stack,
        data: &mut HashMap<Mime, String>,
        mime: &str,
    ) -> bool {
        let pipeline_data = PipelineData::Value(value, None);
        let call = Call {
            decl_id,
            head: Span::unknown(),
            arguments: vec![],
            parser_info: HashMap::new(),
        };
        let formatted = match nu_engine::eval_call::<WithoutDebug>(
            &engine_state,
            stack,
            &call,
            pipeline_data,
        ) {
            Err(_) => return false,
            Ok(formatted) => formatted,
        };
        let formatted = formatted
            .into_value(Span::unknown())
            .into_string()
            .expect("formatted to string");
        let mime = mime.parse().expect("should be valid mime");
        data.insert(mime, formatted);
        return true;
    }

    pub fn render(
        pipeline_data: PipelineData,
        engine_state: &EngineState,
        stack: &mut Stack,
        to_decl_ids: ToDeclIds,
    ) -> PipelineRender {
        let mut data = HashMap::new();
        let metadata = HashMap::new();
        // TODO: use real span here
        let value = pipeline_data.into_value(Span::unknown());
        let ty = value.get_type();

        // `to text` has any input type, no need to check
        Self::render_via_call(
            value.clone(),
            to_decl_ids.to_text,
            engine_state,
            stack,
            &mut data,
            "text/plain",
        );
        // TODO: pass args to disable theme
        // call directly as `ToHtml` is private
        Self::render_via_call(
            value.clone(),
            to_decl_ids.to_html,
            engine_state,
            stack,
            &mut data,
            "text/html",
        );
        Self::render_via_cmd(
            &value,
            ToCsv,
            to_decl_ids.to_csv,
            engine_state,
            stack,
            &mut data,
            "text/csv",
        );
        Self::render_via_cmd(
            &value,
            ToJson,
            to_decl_ids.to_json,
            engine_state,
            stack,
            &mut data,
            "application/json",
        );
        Self::render_via_cmd(
            &value,
            ToMd,
            to_decl_ids.to_md,
            engine_state,
            stack,
            &mut data,
            "text/markdown",
        );

        PipelineRender { data, metadata }
    }
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
