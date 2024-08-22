use nu_engine::command_prelude::*;
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{FromValue, LabeledError};
use plotters::chart::ChartBuilder;
use plotters::prelude::{IntoDrawingArea, SVGBackend};
use plotters::series::LineSeries;
use plotters::style::{RGBAColor, ShapeStyle};

use crate::value::{self, Coord2d, Series2d};

#[derive(Debug, Clone)]
pub struct DrawSvg;

impl Command for DrawSvg {
    fn name(&self) -> &str {
        "draw svg"
    }

    fn signature(&self) -> Signature {
        Signature::new(Command::name(self))
            .add_help()
            .usage(Command::usage(self))
            .search_terms(
                Command::search_terms(self)
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect(),
            )
            .input_output_type(value::Chart2d::ty(), Type::String)
    }

    fn usage(&self) -> &str {
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

        {
            if chart.series.is_empty() {
                todo!("return some error that empty series do not work")
            }

            let x_spec = chart.x_range().map(|(min, max)| min..max).expect("not empty");
            let y_spec = chart.y_range().map(|(min, max)| min..max).expect("not empty");

            let drawing_area = SVGBackend::with_string(&mut output, (chart.width, chart.height))
                .into_drawing_area();
            if let Some(color) = chart.background {
                let color: RGBAColor = color.into();
                drawing_area.fill(&color).unwrap();
            }

            let mut chart_builder = ChartBuilder::on(&drawing_area);
            let mut chart_context = chart_builder
                .build_cartesian_2d(x_spec, y_spec)
                .unwrap();
            chart_context.configure_mesh().draw().unwrap();
            for Series2d {
                series,
                style,
                color,
                filled,
                stroke_width,
                point_size,
            } in chart.series
            {
                match style {
                    value::Series2dStyle::Line => {
                        let series = LineSeries::new(
                            series.into_iter().map(|Coord2d { x, y }| (x, y)),
                            ShapeStyle {
                                color: color.into(),
                                filled,
                                stroke_width,
                            },
                        )
                        .point_size(point_size);
                        chart_context.draw_series(series).unwrap();
                    }
                }
            }
        }

        Ok(Value::string(output, span))
    }
}
