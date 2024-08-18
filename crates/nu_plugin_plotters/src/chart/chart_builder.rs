use nu_protocol::{FromValue, IntoValue};
use plotters::chart::ChartBuilder;
use plotters::prelude::{IntoDrawingArea, SVGBackend};
use plotters::style::WHITE;
use serde::{Deserialize, Serialize};

// TODO: add more fields to represent ChartBuilder more accurately
// TODO: add a background color
// TODO: maybe replace public fields with setter methods
#[derive(Debug, Clone, IntoValue, FromValue, Serialize, Deserialize)]
pub struct ChartBuilderOptions {
    pub size: (u32, u32), // technically part of the drawing area
    pub caption: Option<String>,
    pub margin: [u32; 4],
}

impl ChartBuilderOptions {
    pub fn with_chart_builder<F>(&self, chart_op: F) -> String
    where
        F: FnOnce(&mut ChartBuilder<SVGBackend>),
    {
        let mut buf = String::new();
        {
            let drawing_area = SVGBackend::with_string(&mut buf, self.size).into_drawing_area();
            drawing_area
                .fill(&WHITE)
                .expect("std::io::Error infallible on String");

            let mut chart_builder = ChartBuilder::on(&drawing_area);
            chart_builder
                .margin_left(self.margin[0])
                .margin_bottom(self.margin[1])
                .margin_right(self.margin[2])
                .margin_top(self.margin[3]);

            if let Some(caption) = &self.caption {
                // TODO: check if this should be it or if we should have this configurable
                chart_builder.caption(caption, ("sans-serif", 20));
            }

            chart_op(&mut chart_builder);
        }
        buf
    }
}

super::impl_custom_value!(ChartBuilderOptions);
