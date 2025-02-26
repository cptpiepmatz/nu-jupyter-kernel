use std::collections::HashMap;

use mime::Mime;
use nu_command::{ToCsv, ToJson, ToMd};
use nu_plotters::commands::draw::DrawSvg;
use nu_protocol::ast::{Argument, Call};
use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{Command, EngineState, Stack};
use nu_protocol::{DeclId, PipelineData, ShellError, Span, Spanned, Value};
use thiserror::Error;

use super::module::KernelInternalSpans;
use crate::error::KernelError;

macro_rules! create_format_decl_ids {
    ($($field:ident : $search_str:expr),+ $(,)?) => {
        #[derive(Debug, Clone, Copy)]
        pub struct FormatDeclIds {
            $(pub $field: DeclId,)+
        }

        impl FormatDeclIds {
            pub fn find(engine_state: &EngineState) -> Result<FormatDeclIds, KernelError> {
                $(let mut $field = None;)+

                for (str_bytes, decl_id) in engine_state.get_decls_sorted(false) {
                    let Ok(s) = String::from_utf8(str_bytes) else { continue };
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

                let mut missing = Vec::new();
                $(if $field.is_none() { missing.push($search_str) })+
                Err(KernelError::MissingFormatDecls {missing})
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
    // TODO: make this feature flagged
    draw_svg: "draw svg"
);

fn flag(flag: impl Into<String>, span: Span) -> Argument {
    Argument::Named((
        Spanned {
            item: flag.into(),
            span,
        },
        None,
        None,
    ))
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("could not convert pipeline data into value: {0}")]
    IntoValue(#[source] ShellError),

    #[error("could not render plain text output: {0}")]
    NoText(#[source] ShellError),
}

#[derive(Debug)]
enum InternalRenderError {
    Eval(ShellError),
    IntoValue(ShellError),
    NoString(ShellError),
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
        decl_id: DeclId,
        engine_state: &EngineState,
        span: Span,
        stack: &mut Stack,
    ) -> Result<Option<String>, InternalRenderError> {
        let ty = value.get_type();
        let may = to_cmd
            .signature()
            .input_output_types
            .iter()
            .map(|(input, _)| input)
            .any(|input| ty.is_subtype_of(input));

        match may {
            false => Ok(None),
            true => {
                Self::render_via_call(value.clone(), decl_id, engine_state, stack, span, vec![])
                    .map(Option::Some)
            }
        }
    }

    fn render_via_call(
        value: Value,
        decl_id: DeclId,
        engine_state: &EngineState,
        stack: &mut Stack,
        span: Span,
        arguments: Vec<Argument>,
    ) -> Result<String, InternalRenderError> {
        let pipeline_data = PipelineData::Value(value, None);
        let call = Call {
            decl_id,
            head: span,
            arguments,
            parser_info: HashMap::new(),
        };
        let formatted =
            nu_engine::eval_call::<WithoutDebug>(engine_state, stack, &call, pipeline_data)
                .map_err(InternalRenderError::Eval)?;
        let formatted = formatted
            .into_value(Span::unknown())
            .map_err(InternalRenderError::IntoValue)?
            .into_string()
            .map_err(InternalRenderError::NoString)?;
        Ok(formatted)
    }

    pub fn render(
        pipeline_data: PipelineData,
        engine_state: &EngineState,
        stack: &mut Stack,
        spans: &KernelInternalSpans,
        format_decl_ids: FormatDeclIds,
        filter: Option<Mime>,
    ) -> Result<PipelineRender, RenderError> {
        let mut data = HashMap::new();
        let metadata = HashMap::new();
        let value = pipeline_data
            .into_value(Span::unknown())
            .map_err(RenderError::IntoValue)?;
        let ty = value.get_type();

        // `to text` has any input type, no need to check
        // also we always need to provide plain text output
        match Self::render_via_call(
            value.clone(),
            format_decl_ids.to_text,
            engine_state,
            stack,
            spans.render.text,
            vec![],
        ) {
            Ok(s) => data.insert(mime::TEXT_PLAIN, s),
            Err(
                InternalRenderError::Eval(e) |
                InternalRenderError::IntoValue(e) |
                InternalRenderError::NoString(e),
            ) => return Err(RenderError::NoText(e)),
        };

        let match_filter = |mime| filter.is_none() || filter == Some(mime);

        // call directly as `ToHtml` is private
        if match_filter(mime::TEXT_HTML) {
            let span = spans.render.html;
            match Self::render_via_call(
                value.clone(),
                format_decl_ids.to_html,
                engine_state,
                stack,
                span,
                vec![flag("partial", span), flag("html-color", span)],
            ) {
                Ok(s) => data.insert(mime::TEXT_HTML, s),
                Err(InternalRenderError::Eval(_)) => None,
                Err(_) => None, // TODO: print the error
            };
        }

        if match_filter(mime::TEXT_CSV) {
            match Self::render_via_cmd(
                &value,
                ToCsv,
                format_decl_ids.to_csv,
                engine_state,
                spans.render.csv,
                stack,
            ) {
                Ok(Some(s)) => data.insert(mime::TEXT_CSV, s),
                Ok(None) | Err(InternalRenderError::Eval(_)) => None,
                Err(_) => None, // TODO: print the error
            };
        }

        if match_filter(mime::APPLICATION_JSON) {
            match Self::render_via_cmd(
                &value,
                ToJson,
                format_decl_ids.to_json,
                engine_state,
                spans.render.json,
                stack,
            ) {
                Ok(Some(s)) => data.insert(mime::APPLICATION_JSON, s),
                Ok(None) | Err(InternalRenderError::Eval(_)) => None,
                Err(_) => None, // TODO: print the error
            };
        }

        let md_mime: mime::Mime = "text/markdown"
            .parse()
            .expect("'text/markdown' is valid mime type");
        if match_filter(md_mime.clone()) {
            match Self::render_via_cmd(
                &value,
                ToMd,
                format_decl_ids.to_md,
                engine_state,
                spans.render.md,
                stack,
            ) {
                Ok(Some(s)) => data.insert(md_mime, s),
                Ok(None) | Err(InternalRenderError::Eval(_)) => None,
                Err(_) => None, // TODO: print the error
            };
        }

        // TODO: feature flag this
        if match_filter(mime::IMAGE_SVG) {
            match Self::render_via_cmd(
                &value,
                DrawSvg,
                format_decl_ids.draw_svg,
                engine_state,
                spans.render.svg,
                stack,
            ) {
                Ok(Some(s)) => data.insert(mime::IMAGE_SVG, s),
                Ok(None) | Err(InternalRenderError::Eval(_)) => None,
                Err(_) => None, // TODO: print the error
            };
        }

        Ok(PipelineRender { data, metadata })
    }
}

#[derive(Debug)]
pub struct StringifiedPipelineRender {
    pub data: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}

impl From<PipelineRender> for StringifiedPipelineRender {
    fn from(render: PipelineRender) -> Self {
        Self {
            data: render
                .data
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            metadata: render
                .metadata
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }
}
