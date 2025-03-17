use icy_sixel::{DiffusionMethod, MethodForLargest, MethodForRep, PixelFormat, Quality};
use image::{DynamicImage, ImageBuffer, RgbImage};
use nu_engine::command_prelude::*;
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{FromValue, LabeledError};
use plotters::prelude::{BitMapBackend, IntoDrawingArea};
use plotters::style::WHITE;
use viuer::KittySupport;

use crate::value;

#[derive(Debug, Clone)]
pub struct DrawTerminal;

impl Command for DrawTerminal {
    fn name(&self) -> &str {
        "draw terminal"
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
            .input_output_type(value::Chart2d::ty(), Type::Nothing)
    }

    fn description(&self) -> &str {
        "Draws a chart to a sixel string. Compatible terminal emulators may render that."
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["plotters", "chart", "2d", "draw", "terminal"]
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
        DrawTerminal::run(self, input).map(|v| PipelineData::Value(v, None))
    }
}

impl SimplePluginCommand for DrawTerminal {
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
        DrawTerminal::run(self, input).map_err(Into::into)
    }
}

impl DrawTerminal {
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

        // TODO: convert these errors somehow into shell errors
        if viuer::get_kitty_support() != KittySupport::None || viuer::is_iterm_supported() {
            let img: RgbImage = ImageBuffer::from_raw(size.0, size.1, buf).unwrap();
            let img = DynamicImage::ImageRgb8(img);
            viuer::print(&img, &viuer::Config::default()).unwrap();
        }
        else {
            let sixel = icy_sixel::sixel_string(
                &buf,
                size.0 as i32,
                size.1 as i32,
                PixelFormat::RGB888,
                DiffusionMethod::Stucki,
                MethodForLargest::Auto,
                MethodForRep::Auto,
                Quality::HIGH,
            )
            .unwrap();
            println!("{sixel}");
        }

        Ok(Value::nothing(span))
    }
}
