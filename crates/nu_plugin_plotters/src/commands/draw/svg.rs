use nu_engine::command_prelude::*;
use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{FromValue, LabeledError, PipelineMetadata};
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
        call: &Call,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        Ok(DrawSvg::run(self, input, call.head)?)
    }
}

impl PluginCommand for DrawSvg {
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
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        Ok(DrawSvg::run(self, input, call.head)?)
    }
}

impl DrawSvg {
    fn run(&self, input: PipelineData, span: Span) -> Result<PipelineData, ShellError> {
        let input = input.into_value(span)?;
        let span = input.span();
        let chart = value::Chart2d::from_value(input)?;

        let mut output = String::new();
        let drawing_backend = SVGBackend::with_string(&mut output, (chart.width, chart.height));
        super::draw(chart, drawing_backend.into_drawing_area());

        Ok(PipelineData::Value(
            Value::string(output, span),
            Some(
                PipelineMetadata::default()
                    .with_content_type(Some(mime2::image::SVG_XML.to_string())),
            ),
        ))
    }
}
