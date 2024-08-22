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
                value::Chart2d::ty().to_shape(),
                "Baseline chart to extend from.",
            )
            .named(
                "width",
                SyntaxShape::Int,
                "Set the width of the chart in pixels.",
                Some('W'),
            )
            .named(
                "height",
                SyntaxShape::Int,
                "Set the height of the chart in pixels.",
                Some('H'),
            )
            .named(
                "background-color",
                SyntaxShape::Any,
                "Set the background color of the chart.",
                Some('b'),
            )
            .named(
                "caption",
                SyntaxShape::String,
                "Set a caption for the chart.",
                Some('c'),
            )
            .named(
                "margin",
                SyntaxShape::List(Box::new(SyntaxShape::Int)),
                "Set the margin for the chart, refer to css margin shorthands for setting values.",
                Some('m'),
            )
            .named(
                "label-area",
                SyntaxShape::List(Box::new(SyntaxShape::Int)),
                "Set the area size for the chart, refer to css margin shorthands for setting values.",
                Some('l')
            )
            .input_output_type(Type::Nothing, value::Chart2d::ty())
            .input_output_type(value::Series2d::ty(), value::Chart2d::ty())
            .input_output_type(Type::list(value::Series2d::ty()), value::Chart2d::ty())
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
        let margin = call.get_flag(engine_state, stack, "margin")?;
        let label_area = call.get_flag(engine_state, stack, "label-area")?;
        Chart2d::run(self, input, extend, Chart2dOptions {width, height, background, caption, margin, label_area })
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
            .map(|v| <value::Chart2d>::from_value(v.clone()))
            .transpose()?;

        let mut options = Chart2dOptions::default();
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
                "width" => options.width = extract_named("width", value, name.span)?,
                "height" => options.height = extract_named("height", value, name.span)?,
                "background" => options.background = extract_named("background", value, name.span)?,
                "caption" => options.caption = extract_named("caption", value, name.span)?,
                "margin" => options.margin = extract_named("margin", value, name.span)?,
                "label-area" => options.label_area = extract_named("label-area", value, name.span)?,
                _ => continue,
            }
        }

        Chart2d::run(self, input, extend, options).map_err(Into::into)
    }
}

#[derive(Debug, Default)]
struct Chart2dOptions {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub background: Option<value::Color>,
    pub caption: Option<String>,
    pub margin: Option<Vec<u32>>,
    pub label_area: Option<Vec<u32>>,
}

impl Chart2d {
    fn run(
        &self,
        input: Value,
        extend: Option<value::Chart2d>,
        options: Chart2dOptions,
    ) -> Result<Value, ShellError> {
        let span = input.span();
        let mut input = match input {
            v @ Value::Custom { .. } => vec![value::Series2d::from_value(v)?],
            v @ Value::List { .. } => Vec::from_value(v)?,
            _ => todo!("handle invalid input"),
        };

        let mut chart = extend.unwrap_or_default();
        chart.series.append(&mut input);
        chart.width = options.width.unwrap_or(chart.width);
        chart.height = options.height.unwrap_or(chart.height);
        chart.background = options.background.or(chart.background);
        chart.caption = options.caption.or(chart.caption);
        chart.margin = options.margin.map(side_shorthand).transpose()?.unwrap_or(chart.margin);
        chart.label_area = options.label_area.map(side_shorthand).transpose()?.unwrap_or(chart.label_area);
        
        Ok(Value::custom(Box::new(chart), span))
    }
}

fn side_shorthand<T: Copy>(input: Vec<T>) -> Result<[T; 4], ShellError> {
    let mut iter = input.into_iter();
    Ok(match (iter.next(), iter.next(), iter.next(), iter.next()) {
        (Some(a), None, None, None) => [a, a, a, a],
        (Some(a), Some(b), None, None) => [a, b, a, b],
        (Some(a), Some(b), Some(c), None) => [a, b, b, c],
        (Some(a), Some(b), Some(c), Some(d)) => [a, b, c, d],
        (None, None, None, None) => todo!("throw error for empty list"),
        _ => unreachable!("all other variants are not possible with lists")
    })
}
