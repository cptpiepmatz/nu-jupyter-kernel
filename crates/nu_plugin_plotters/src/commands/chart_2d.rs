use nu_engine::command_prelude::*;
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{FromValue, IntoValue, LabeledError};

use crate::value;

#[derive(Debug, Clone)]
pub struct Chart2d;

impl Command for Chart2d {
    fn name(&self) -> &str {
        "chart 2d"
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
            .optional(
                "chart",
                value::chart_2d::Chart2d::ty().to_shape(),
                "Baseline chart to extend from.",
            )
            .named(
                "width",
                SyntaxShape::Int,
                "Set the width of the chart in pixels.",
                Some('w'),
            )
            .named(
                "height",
                SyntaxShape::Int,
                "Set the height of the chart in pixels.",
                Some('h'),
            )
            .named(
                "background-color",
                value::color::Color::syntax_shape(),
                "Set the background color of the chart.",
                Some('b'),
            )
            .named(
                "caption",
                SyntaxShape::String,
                "Set a caption for the chart.",
                Some('c'),
            )
            .input_output_type(Type::Nothing, value::chart_2d::Chart2d::ty())
            .input_output_type(
                value::series_2d::Series2d::ty(),
                value::chart_2d::Chart2d::ty(),
            )
            .input_output_type(
                Type::list(value::series_2d::Series2d::ty()),
                value::chart_2d::Chart2d::ty(),
            )
    }

    fn usage(&self) -> &str {
        "Construct a 2D chart."
    }

    fn extra_usage(&self) -> &str {
        "A chart is a container for a list of series, any `plotters::series-2d` or \
         `list<plotters::series>` is collected into this container and may be rendered via `draw \
         svg` or `draw png`."
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["plotters", "chart", "2d"]
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
        let extend = call.opt(engine_state, stack, 0)?;
        let width = call.get_flag(engine_state, stack, "width")?;
        let height = call.get_flag(engine_state, stack, "height")?;
        let background = call.get_flag(engine_state, stack, "background")?;
        let caption = call.get_flag(engine_state, stack, "caption")?;
        Chart2d::run(self, input, extend, width, height, background, caption)
            .map(|v| PipelineData::Value(v, None))
    }
}

impl SimplePluginCommand for Chart2d {
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
        let extend = call
            .positional
            .first()
            .map(|v| <value::chart_2d::Chart2d>::from_value(v.clone()))
            .transpose()?;
        let (mut width, mut height, mut background, mut caption) = Default::default();
        for (name, value) in call.named.clone() {
            // TODO: put this function somewhere reusable
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
                "width" => width = extract_named("width", value, name.span)?,
                "height" => height = extract_named("height", value, name.span)?,
                "background" => background = extract_named("background", value, name.span)?,
                "caption" => caption = extract_named("caption", value, name.span)?,
                _ => continue,
            }
        }

        Chart2d::run(self, input, extend, width, height, background, caption).map_err(Into::into)
    }
}

impl Chart2d {
    fn run(
        &self,
        input: Value,
        extend: Option<value::chart_2d::Chart2d>,
        width: Option<u32>,
        height: Option<u32>,
        background: Option<value::color::Color>,
        caption: Option<String>,
    ) -> Result<Value, ShellError> {
        let span = input.span();
        let mut input = match input {
            v @ Value::Custom { .. } => vec![value::series_2d::Series2d::from_value(v)?],
            v @ Value::List { .. } => Vec::from_value(v)?,
            _ => todo!("handle invalid input")
        };

        let mut chart = extend.unwrap_or_default();
        chart.series.append(&mut input);
        chart.width = width.unwrap_or(chart.width);
        chart.height = height.unwrap_or(chart.height);
        chart.background = background.or(chart.background);
        chart.caption = caption.or(chart.caption);
        Ok(chart.into_value(span))
    }
}
