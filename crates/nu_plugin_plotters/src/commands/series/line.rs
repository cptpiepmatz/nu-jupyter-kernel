use nu_engine::command_prelude::*;
use nu_plugin::{EngineInterface, EvaluatedCall};
use nu_protocol::{FromValue, IntoValue, LabeledError};
use plotters::style::BLUE;

use crate::value::{Color, Line2dSeries, Series2d};

#[derive(Debug, Clone)]
pub struct LineSeries;

impl Command for LineSeries {
    fn name(&self) -> &str {
        "series line"
    }

    fn signature(&self) -> Signature {
        Signature::new(Command::name(self))
            .add_help()
            .description(Command::description(self))
            .extra_description(Command::extra_description(self))
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
                "point-size",
                SyntaxShape::Int,
                "Define the size of the points in pixels.",
                Some('p'),
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

    fn description(&self) -> &str {
        "Create a line series."
    }

    fn extra_description(&self) -> &str {
        "This series requires as input a list or stream of value pairs for the x and y axis."
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["plotters", "series", "line", "chart"]
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
        let point_size = call.get_flag(engine_state, stack, "point-size")?;
        LineSeries::run(self, input, color, filled, stroke_width, point_size)
            .map(|v| PipelineData::Value(v, None))
    }
}

impl nu_plugin::SimplePluginCommand for LineSeries {
    type Plugin = crate::plugin::PlottersPlugin;

    fn name(&self) -> &str {
        Command::name(self)
    }

    fn signature(&self) -> Signature {
        Command::signature(self)
    }

    fn description(&self) -> &str {
        Command::description(self)
    }

    fn extra_description(&self) -> &str {
        Command::extra_description(self)
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
        let (mut color, mut filled, mut stroke_width, mut point_size) = Default::default();
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
                "point-size" => point_size = extract_named("point-size", value, name.span)?,
                _ => continue,
            }
        }

        LineSeries::run(self, input, color, filled, stroke_width, point_size).map_err(Into::into)
    }
}

impl LineSeries {
    fn run(
        &self,
        input: Value,
        color: Option<Color>,
        filled: Option<bool>,
        stroke_width: Option<u32>,
        point_size: Option<u32>,
    ) -> Result<Value, ShellError> {
        let input_span = input.span();
        let series = super::series_from_value(input)?;
        let series = Line2dSeries {
            series,
            color: color.unwrap_or(BLUE.into()),
            filled: filled.unwrap_or(false),
            stroke_width: stroke_width.unwrap_or(1),
            point_size: point_size.unwrap_or(0),
        };

        Ok(Series2d::Line(series).into_value(input_span))
    }
}
