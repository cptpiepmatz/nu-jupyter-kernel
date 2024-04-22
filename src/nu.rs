use std::collections::HashMap;

use mime::Mime;
use nu_command::{ToCsv, ToJson, ToMd, ToText};
use nu_protocol::ast::Call;
use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{Command, EngineState, Stack, StateWorkingSet};
use nu_protocol::{PipelineData, Span, Value};

static INPUT: &str = "{a: 3} | to json";

pub fn initial_engine_state() -> EngineState {
    // TODO: compare with nu_cli::get_engine_state for other contexts
    let engine_state = nu_cmd_lang::create_default_context();
    let engine_state = nu_command::add_shell_command_context(engine_state);
    let engine_state = nu_cmd_extra::add_extra_command_context(engine_state);
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
    fn render_data_type(
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
        if may {
            let pipeline_data = PipelineData::Value(value.clone(), None);
            let call = Call {
                decl_id,
                head: Span::unknown(),
                arguments: vec![],
                parser_info: HashMap::new(),
            };
            let formatted = to_cmd
                .run(&engine_state, stack, &call, pipeline_data)
                .unwrap();
            let formatted = formatted
                .into_value(Span::unknown())
                .into_string()
                .expect("formatted to string");
            let mime = mime.parse().expect("should be valid mime");
            data.insert(mime, formatted);
        }
        may
    }

    pub fn render(
        pipeline_data: PipelineData,
        engine_state: &EngineState,
        stack: &mut Stack,
        to_decl_ids: ToDeclIds,
    ) -> PipelineRender {
        let mut data = HashMap::new();
        let mut metadata = HashMap::new();
        // TODO: use real span here
        let value = pipeline_data.into_value(Span::unknown());
        let ty = value.get_type();

        Self::render_data_type(
            &value,
            ToText,
            to_decl_ids.to_text,
            engine_state,
            stack,
            &mut data,
            "text/plain",
        );
        Self::render_data_type(
            &value,
            ToCsv,
            to_decl_ids.to_csv,
            engine_state,
            stack,
            &mut data,
            "text/csv",
        );
        Self::render_data_type(
            &value,
            ToJson,
            to_decl_ids.to_json,
            engine_state,
            stack,
            &mut data,
            "application/json",
        );
        Self::render_data_type(
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

pub fn execute(
    code: &str,
    engine_state: &mut EngineState,
    stack: &mut Stack,
) -> Result<PipelineData, ()> {
    let code = code.as_bytes();
    let mut working_set = StateWorkingSet::new(engine_state);
    // TODO: use for fname the history counter
    let block = nu_parser::parse(&mut working_set, None, code, false);

    if let Some(error) = working_set.parse_errors.first() {
        todo!("handle parsing errors");
    }

    engine_state.merge_delta(working_set.delta).unwrap();
    let res =
        nu_engine::eval_block::<WithoutDebug>(engine_state, stack, &block, PipelineData::Empty)
            .unwrap(); // TODO: handle evaluation errors somehow
    Ok(res)
}
