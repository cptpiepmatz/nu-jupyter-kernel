use std::io::Cursor;

use image::{DynamicImage, ImageBuffer, ImageFormat, RgbImage};
use nu_engine::command_prelude::*;
use nu_plugin::PluginCommand;
use nu_protocol::{FromValue, PipelineMetadata};
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
        DrawPng::run(self, input, call.head)
    }
}

impl PluginCommand for DrawPng {
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
        call: &nu_plugin::EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, nu_protocol::LabeledError> {
        Ok(DrawPng::run(self, input, call.head)?)
    }
}

impl DrawPng {
    const CONTENT_TYPE: mime2::Mime = mime2::image::PNG;

    fn run(&self, input: PipelineData, span: Span) -> Result<PipelineData, ShellError> {
        let input = input.into_value(span)?;
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

        Ok(PipelineData::Value(
            Value::binary(png_buf.into_inner(), span),
            Some(
                PipelineMetadata::default().with_content_type(Some(Self::CONTENT_TYPE.to_string())),
            ),
        ))
    }
}
