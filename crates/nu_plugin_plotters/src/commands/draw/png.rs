use std::io::Cursor;

use image::{DynamicImage, ImageBuffer, ImageFormat, RgbImage};
use nu_engine::command_prelude::*;
use nu_plugin::SimplePluginCommand;
use nu_protocol::FromValue;
use plotters::prelude::{BitMapBackend, IntoDrawingArea};
use plotters::style::WHITE;

use crate::value;

#[derive(Debug, Clone)]
pub struct DrawPng;

impl Command for DrawPng {
    fn name(&self) -> &str {
        "draw png"
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
            .input_output_type(value::Chart2d::ty(), Type::Binary)
    }

    fn description(&self) -> &str {
        "Draws a chart on a PNG byte buffer."
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["plotters", "chart", "2d", "draw", "png"]
    }

    fn run(
        &self,
        _: &EngineState,
        _: &mut Stack,
        call: &Call,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        let span = input.span().unwrap_or(call.head);
        let input = input.into_value(span)?;
        DrawPng::run(self, input).map(|v| PipelineData::Value(v, None))
    }
}

impl SimplePluginCommand for DrawPng {
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
        _: &nu_plugin::EngineInterface,
        _: &nu_plugin::EvaluatedCall,
        input: &Value,
    ) -> Result<Value, nu_protocol::LabeledError> {
        let input = input.clone();
        DrawPng::run(self, input).map_err(Into::into)
    }
}

impl DrawPng {
    fn run(&self, input: Value) -> Result<Value, ShellError> {
        let span = input.span();
        let mut chart = value::Chart2d::from_value(input)?;

        const BYTES_PER_PIXEL: u32 = 3;
        let bytes = chart.height * chart.width * BYTES_PER_PIXEL;
        let mut buf: Vec<u8> = vec![0; bytes as usize];

        let size = (chart.width, chart.height);
        let drawing_backend = BitMapBackend::with_buffer(&mut buf, size);

        if chart.background.is_none() {
            chart.background = Some(WHITE.into());
        }

        super::draw(chart, drawing_backend.into_drawing_area());

        let img: RgbImage = ImageBuffer::from_raw(size.0, size.1, buf).unwrap();
        let img = DynamicImage::ImageRgb8(img);
        let mut png_buf = Cursor::new(Vec::new());
        img.write_to(&mut png_buf, ImageFormat::Png).unwrap();

        Ok(Value::binary(png_buf.into_inner(), span))
    }
}
