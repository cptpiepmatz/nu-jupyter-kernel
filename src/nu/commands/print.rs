use std::collections::HashMap;

use mime_guess::MimeGuess;
use nu_engine::CallExt;
use nu_protocol::engine::Command;
use nu_protocol::{FromValue, PipelineData, ShellError, Signature, Span, SyntaxShape, Type, Value};

use super::{JupyterCommandContext, COMMANDS_TOML};
use crate::jupyter::messages::iopub::{DisplayData, IopubBroacast};
use crate::jupyter::messages::{Header, Message, Metadata};
use crate::nu::render::{PipelineRender, StringifiedPipelineRender};

#[derive(Debug, Clone)]
pub struct Print(JupyterCommandContext);

impl Print {
    pub fn new(ctx: JupyterCommandContext) -> Self {
        Self(ctx)
    }
}

impl Command for Print {
    fn name(&self) -> &str {
        COMMANDS_TOML.print.name
    }

    fn usage(&self) -> &str {
        COMMANDS_TOML.print.usage
    }

    fn search_terms(&self) -> Vec<&str> {
        COMMANDS_TOML.print.search_terms.into()
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .optional("input", SyntaxShape::Any, "Value to print")
            .named(
                "format",
                SyntaxShape::String,
                "Format to filter for",
                Some('f'),
            )
            .input_output_types(vec![
                (Type::Any, Type::Nothing),
                (Type::Nothing, Type::Nothing),
            ])
            .category(super::category())
    }

    // TODO: split this into multiple parts, this is too much
    fn run(
        &self,
        engine_state: &nu_protocol::engine::EngineState,
        stack: &mut nu_protocol::engine::Stack,
        call: &nu_protocol::engine::Call,
        input: nu_protocol::PipelineData,
    ) -> Result<nu_protocol::PipelineData, ShellError> {
        let arg: Option<Value> = call.opt(engine_state, stack, 0)?;
        let arg: Option<PipelineData> = arg.map(|v| PipelineData::Value(v, None));
        let input_span = input.span(); // maybe needed for an error
        let to_render = match (input, arg) {
            // no data provided, throw error
            (PipelineData::Empty, None) => Err(ShellError::GenericError {
                error: "No input data".to_string(),
                msg: "No data was piped or passed as an argument to the command.".to_string(),
                span: Some(call.span()),
                help: Some(
                    "Please provide data through the pipeline or as an argument.".to_string(),
                ),
                inner: vec![],
            }),

            // passed arg has no data, throw error
            (_, Some(PipelineData::Empty)) => Err(ShellError::TypeMismatch {
                err_message: "Expected non-empty data, but found empty".to_string(),
                span: call.arguments_span(),
            }),

            // render passed arg
            (PipelineData::Empty, Some(data)) => Ok(data),

            // too many inputs, throw error
            (_, Some(_)) => Err(ShellError::IncompatibleParameters {
                left_message: "Either pass data via pipe".to_string(),
                left_span: input_span.unwrap_or(call.head),
                right_message: "Or pass data via an argument".to_string(),
                right_span: call.arguments_span(),
            }),

            // render piped arg
            (data, None) => Ok(data),
        }?;

        let format: Option<Value> = call.get_flag(engine_state, stack, "format")?;
        let spanned_format: Option<(Span, Value)> = format.map(|v| (v.span(), v));
        let spanned_format: Option<(Span, String)> = spanned_format
            .map(|(span, v)| String::from_value(v).map(|s| (span, s)))
            .transpose()?;
        let mime = spanned_format
            .map(|(span, s)| {
                MimeGuess::from_ext(&s)
                    .first()
                    .ok_or_else(|| ShellError::IncorrectValue {
                        msg: "Cannot guess a mime type".to_owned(),
                        val_span: span,
                        call_span: call.head,
                    })
            })
            .transpose()?;

        let render: StringifiedPipelineRender =
            PipelineRender::render(to_render, engine_state, stack, self.0.format_decl_ids, mime)
                .unwrap() // TODO: handle this better
                .into();

        let display_data = DisplayData {
            data: render.data,
            metadata: render.metadata,
            transient: HashMap::new(),
        };
        let broadcast = IopubBroacast::DisplayData(display_data);

        let konst = self.0.konst.data(stack, call.head)?;
        let message = Message {
            zmq_identities: konst.message.zmq_identities,
            header: Header::new(broadcast.msg_type()),
            parent_header: Some(konst.message.header),
            metadata: Metadata::empty(),
            content: broadcast,
            buffers: vec![],
        };

        self.0
            .iopub
            .send(message.into_multipart().unwrap())
            .unwrap();

        Ok(PipelineData::Empty)
    }
}
