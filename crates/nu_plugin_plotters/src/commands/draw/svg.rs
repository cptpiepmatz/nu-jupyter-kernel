use nu_engine::command_prelude::*;
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{FromValue, LabeledError};
use plotters::prelude::{IntoDrawingArea, SVGBackend};

use crate::value;

#[derive(Debug, Clone)]
pub struct DrawSvg;

impl Command for DrawSvg {
    fn name(&self) -> &str {
        "draw svg"
    }

    fn signature(&self) -> Signature {
        Signature::new(Command::name(self))
            .add_help()
            .description(Command::description(self))
            .search_terms(
                Command::search_terms(self)
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect(),
            )
            .input_output_type(value::Chart2d::ty(), Type::String)
    }

    fn description(&self) -> &str {
        "Draws a chart on a SVG string."
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["plotters", "chart", "2d", "draw", "svg"]
    }

    fn run(
        &self,
        _: &EngineState,
        _: &mut Stack,
        _: &Call,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = input.span().unwrap_or(Span::unknown());
        let input = input.into_value(span)?;
        DrawSvg::run(self, input).map(|v| PipelineData::Value(v, None))
    }
}

impl SimplePluginCommand for DrawSvg {
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

    fn search_terms(&self) -> Vec<&str> {
        Command::search_terms(self)
    }

    fn run(
        &self,
        _: &Self::Plugin,
        _: &EngineInterface,
        _: &EvaluatedCall,
        input: &Value,
    ) -> Result<Value, LabeledError> {
        let input = input.clone();
        DrawSvg::run(self, input).map_err(Into::into)
    }
}

impl DrawSvg {
    fn run(&self, input: Value) -> Result<Value, ShellError> {
        let span = input.span();
        let chart = value::Chart2d::from_value(input)?;

        let mut output = String::new();
        let drawing_backend = SVGBackend::with_string(&mut output, (chart.width, chart.height));
        super::draw(chart, drawing_backend.into_drawing_area());

        Ok(Value::string(output, span))
    }
}
