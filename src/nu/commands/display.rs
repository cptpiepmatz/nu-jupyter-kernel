use const_format::formatcp;
use mime_guess::MimeGuess;
use nu_protocol::engine::Command;
use nu_protocol::{Example, ShellError, Signature, SyntaxShape, Type};

use crate::RENDER_FILTER;

static EXTERNAL_NAME: &str = formatcp!("{} display", super::COMMAND_GROUP);

#[derive(Debug, Clone)]
pub struct Display;

impl Command for Display {
    fn name(&self) -> &str {
        EXTERNAL_NAME
    }

    fn usage(&self) -> &str {
        "Control the rendering of the current cell's output."
    }

    fn extra_usage(&self) -> &str {
        "Applies a filter to control how the output of the current cell is \ndisplayed. This \
         command can be positioned anywhere within the cell's \ncode. It passes through the cell's \
         data, allowing it to be used \neffectively as the final command without altering the \
         output content."
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["jupyter", "display", "cell", "output"]
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required("format", SyntaxShape::String, "Format to filter for")
            .input_output_types(vec![(Type::Any, Type::Any)])
            .category(super::category())
    }

    fn examples(&self) -> Vec<Example> {
        vec![
            Example {
            example: "{a: 3, b: [1, 2, 2]} | nuju display md",
            description: "Force render output to be markdown",
            result: None
        },
        Example {
            example: "{a: 3, b: [1, 2, 2]} | nuju display json",
            description: "Force render output to be json",
            result: None
        },
        Example {
            example: "{a: 3, b: [1, 2, 2]} | table --expand | nuju display txt",
            description: "Force render output to be a classic nushell table",
            result: None
        }
        ]
    }

    fn run(
        &self,
        engine_state: &nu_protocol::engine::EngineState,
        stack: &mut nu_protocol::engine::Stack,
        call: &nu_protocol::ast::Call,
        input: nu_protocol::PipelineData,
    ) -> Result<nu_protocol::PipelineData, ShellError> {
        let format_expr =
            call.positional_iter()
                .next()
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
