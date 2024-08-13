use mime_guess::MimeGuess;
use nu_protocol::engine::Command;
use nu_protocol::{Example, ShellError, Signature, SyntaxShape, Type};

use super::COMMANDS_TOML;
use crate::handlers::shell::RENDER_FILTER;

#[derive(Debug, Clone)]
pub struct Display;

impl Command for Display {
    fn name(&self) -> &str {
        COMMANDS_TOML.display.name
    }

    fn usage(&self) -> &str {
        COMMANDS_TOML.display.usage
    }

    fn extra_usage(&self) -> &str {
        COMMANDS_TOML.display.extra_usage
    }

    fn search_terms(&self) -> Vec<&str> {
        COMMANDS_TOML.display.search_terms.into()
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required("format", SyntaxShape::String, "Format to filter for")
            .input_output_types(vec![(Type::Any, Type::Any)])
            .category(super::category())
    }

    fn examples(&self) -> Vec<Example> {
        COMMANDS_TOML
            .display
            .examples
            .iter()
            .map(|eg| Example {
                example: eg.example,
                description: eg.description,
                result: None,
            })
            .collect()
    }

    fn run(
        &self,
        engine_state: &nu_protocol::engine::EngineState,
        stack: &mut nu_protocol::engine::Stack,
        call: &nu_protocol::engine::Call,
        input: nu_protocol::PipelineData,
    ) -> Result<nu_protocol::PipelineData, ShellError> {
        let format_expr = call.positional_nth(stack, 0)
                .ok_or_else(|| ShellError::MissingParameter {
                    param_name: String::from("format"),
                    span: call.arguments_span(),
                })?;

        let format = format_expr
            .as_string()
            .ok_or_else(|| ShellError::TypeMismatch {
                err_message: "<format> needs to be a string".to_owned(),
                span: format_expr.span,
            })?;

        let mime =
            MimeGuess::from_ext(&format)
                .first()
                .ok_or_else(|| ShellError::IncorrectValue {
                    msg: "cannot guess a mime type".to_owned(),
                    val_span: format_expr.span,
                    call_span: call.head,
                })?;

        RENDER_FILTER.lock().replace(mime);
        Ok(input)
    }
}
