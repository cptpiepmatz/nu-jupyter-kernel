use nu_engine::command_prelude::*;
use nu_plugin::{EngineInterface, EvaluatedCall};
use nu_protocol::{FromValue, LabeledError};
use plotters::style::BLUE;

use crate::value::{Color, Series2d, Series2dStyle};

#[derive(Debug, Clone)]
pub struct BarSeries;

impl Command for BarSeries {
    fn name(&self) -> &str {
        "series bar"
    }

    fn signature(&self) -> Signature {
        Signature::new(Command::name(self))
            .add_help()
            .usage(Command::usage(self))
            .extra_usage(Command::extra_usage(self))
            .search_terms(
                Command::search_terms(self)
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect(),
            )
            .named(
                "color",
                SyntaxShape::Any,
                "Define the color of the points and the line. For valid color inputs, refer to \
                 `plotters colors --help`.",
                Some('c'),
            )
            .named(
                "filled",
                SyntaxShape::Boolean,
                "Define whether the points in the series should be filled.",
                Some('f'),
            )
            .named(
                "stroke-width",
                SyntaxShape::Int,
                "Define the width of the stroke.",
                Some('s'),
            )
            .named(
                "horizontal",
                SyntaxShape::Boolean,
                "Define whether the bars should be horizontal.",
                Some('H'),
            )
            .input_output_type(Type::list(Type::Number), Series2d::ty())
            .input_output_type(Type::list(Type::list(Type::Number)), Series2d::ty())
            .input_output_type(
                Type::list(Type::Record(
                    vec![
                        ("x".to_string(), Type::Number),
                        ("y".to_string(), Type::Number),
                    ]
                    .into_boxed_slice(),
                )),
                Series2d::ty(),
            )
    }

    fn usage(&self) -> &str {
        "Create a bar series."
    }

    fn extra_usage(&self) -> &str {
        "This series requires as input a list or stream of value pairs for the x and y axis."
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["plotters", "series", "bar", "chart"]
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = input.span().unwrap_or(Span::unknown());
        let input = input.into_value(span)?;
        let color = call.get_flag(engine_state, stack, "color")?;
        let filled = call.get_flag(engine_state, stack, "filled")?;
        let stroke_width = call.get_flag(engine_state, stack, "stroke-width")?;
        BarSeries::run(self, input, color, filled, stroke_width)
            .map(|v| PipelineData::Value(v, None))
    }
}

impl nu_plugin::SimplePluginCommand for BarSeries {
    type Plugin = crate::plugin::PlottersPlugin;

    fn name(&self) -> &str {
        Command::name(self)
    }

    fn signature(&self) -> Signature {
        Command::signature(self)
    }

    fn usage(&self) -> &str {
        Command::usage(self)
    }

    fn extra_usage(&self) -> &str {
        Command::extra_usage(self)
    }

    fn search_terms(&self) -> Vec<&str> {
        Command::search_terms(self)
    }

    fn run(
        &self,
        _: &Self::Plugin,
        _: &EngineInterface,
        call: &EvaluatedCall,
        input: &Value,
    ) -> Result<Value, LabeledError> {
        let input = input.clone();
        let (mut color, mut filled, mut stroke_width) = Default::default();
        for (name, value) in call.named.clone() {
            fn extract_named<T: FromValue>(
                name: impl ToString,
                value: Option<Value>,
                span: Span,
            ) -> Result<T, ShellError> {
                let value = value.ok_or_else(|| ShellError::MissingParameter {
                    param_name: name.to_string(),
                    span,
                })?;
                T::from_value(value)
            }

            match name.item.as_str() {
                "color" => color = extract_named("color", value, name.span)?,
                "filled" => filled = extract_named("filled", value, name.span)?,
                "stroke-width" => stroke_width = extract_named("stroke-width", value, name.span)?,
                _ => continue,
            }
        }

        BarSeries::run(self, input, color, filled, stroke_width).map_err(Into::into)
    }
}

impl BarSeries {
    fn run(
        &self,
        input: Value,
        color: Option<Color>,
        filled: Option<bool>,
        stroke_width: Option<u32>,
    ) -> Result<Value, ShellError> {
        let input_span = input.span();
        let series = super::series_from_value(input)?;
        let series = Series2d {
            series,
            style: Series2dStyle::Bar,
            color: color.unwrap_or(BLUE.into()),
            filled: filled.unwrap_or(true),
            stroke_width: stroke_width.unwrap_or(1),
        };

        Ok(Value::custom(Box::new(series), input_span))
    }
}
