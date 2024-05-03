use std::collections::HashMap;

use mime::Mime;
use nu_command::{ToCsv, ToJson, ToMd};
use nu_protocol::ast::{Argument, Call};
use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{Command, EngineState, Stack};
use nu_protocol::{PipelineData, Span, Spanned, Value};

macro_rules! create_format_decl_ids {
    ($($field:ident : $search_str:expr),+ $(,)?) => {
        #[derive(Debug, Clone, Copy)]
        pub struct FormatDeclIds {
            $(pub $field: usize,)+
        }

        impl FormatDeclIds {
            pub fn find(engine_state: &EngineState) -> Result<FormatDeclIds, ()> {
                $(let mut $field = None;)+

                for (str_bytes, decl_id) in engine_state.get_decls_sorted(false) {
                    let s = String::from_utf8(str_bytes).unwrap();
                    match s.as_str() {
                        $($search_str => $field = Some(decl_id),)+
                        _ => (),
                    }
                }

                if let ($(Some($field),)+) = ($($field,)+) {
                    return Ok(FormatDeclIds {
                        $($field,)+
                    });
                }

                todo!("handle not being able to find all formats")
            }
        }
    };
}

create_format_decl_ids!(
    to_text: "to text",
    to_csv: "to csv",
    to_json: "to json",
    to_html: "to html",
    to_md: "to md",
    table: "table",
);

fn flag(flag: impl Into<String>) -> Argument {
    Argument::Named((
        Spanned {
            item: flag.into(),
            span: Span::unknown(),
        },
        None,
        None,
    ))
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
        mime: Mime,
    ) -> bool {
        let ty = value.get_type();
        let may = to_cmd
            .signature()
            .input_output_types
            .iter()
            .map(|(input, _)| input)
            .any(|input| ty.is_subtype(input));
        match may {
            true => Self::render_via_call(
                value.clone(),
                decl_id,
                engine_state,
                stack,
                data,
                vec![],
                mime,
            ),
            false => false,
        }
    }

    fn render_via_call(
        value: Value,
        decl_id: usize,
        engine_state: &EngineState,
        stack: &mut Stack,
        data: &mut HashMap<Mime, String>,
        arguments: Vec<Argument>,
        mime: Mime,
    ) -> bool {
        let pipeline_data = PipelineData::Value(value, None);
        let call = Call {
            decl_id,
            head: Span::unknown(),
            arguments,
            parser_info: HashMap::new(),
        };
        let formatted =
            match nu_engine::eval_call::<WithoutDebug>(engine_state, stack, &call, pipeline_data) {
                Err(_) => return false,
                Ok(formatted) => formatted,
            };
        let formatted = formatted
            .into_value(Span::unknown())
            .into_string()
            .expect("formatted to string");
        data.insert(mime, formatted);
        true
    }

    // TODO: add a render filter here
    pub fn render(
        pipeline_data: PipelineData,
        engine_state: &EngineState,
        stack: &mut Stack,
        format_decl_ids: FormatDeclIds,
        filter: Option<Mime>,
    ) -> PipelineRender {
        let mut data = HashMap::new();
        let metadata = HashMap::new();
        let value = pipeline_data.into_value(Span::unknown());
        let ty = value.get_type();

        // `to text` has any input type, no need to check
        // also we always need to provide plain text output
        Self::render_via_call(
            value.clone(),
            format_decl_ids.to_text,
            engine_state,
            stack,
            &mut data,
            vec![],
            mime::TEXT_PLAIN,
        );

        let match_filter = |mime| filter.is_none() || filter == Some(mime);

        // call directly as `ToHtml` is private
        if match_filter(mime::TEXT_HTML) {
            Self::render_via_call(
                value.clone(),
                format_decl_ids.to_html,
                engine_state,
                stack,
                &mut data,
                vec![flag("partial"), flag("html-color")],
                mime::TEXT_HTML,
            );
        }

        if match_filter(mime::TEXT_CSV) {
            Self::render_via_cmd(
                &value,
                ToCsv,
                format_decl_ids.to_csv,
                engine_state,
                stack,
                &mut data,
                mime::TEXT_CSV,
            );
        }
        if match_filter(mime::APPLICATION_JSON) {
            Self::render_via_cmd(
                &value,
                ToJson,
                format_decl_ids.to_json,
                engine_state,
                stack,
                &mut data,
                mime::APPLICATION_JSON,
            );
        }
        let md_mime: mime::Mime = "text/markdown"
            .parse()
            .expect("'text/markdown' is valid mime type");
        if match_filter(md_mime.clone()) {
            Self::render_via_cmd(
                &value,
                ToMd,
                format_decl_ids.to_md,
                engine_state,
                stack,
                &mut data,
                md_mime,
            );
        }

        PipelineRender { data, metadata }
    }
}
