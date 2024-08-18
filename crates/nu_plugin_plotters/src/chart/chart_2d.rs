use nu_protocol::{FromValue, IntoValue};
use plotters::chart::ChartBuilder;
use plotters::prelude::SVGBackend;
use plotters::series::LineSeries;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, IntoValue, FromValue, Serialize, Deserialize)]
pub struct Chart2dOptions {
    pub x_range: Option<(f64, f64)>,
    pub y_range: Option<(f64, f64)>,
    pub series: Vec<(Vec<(f64, f64)>, SeriesStyle)>,
}

#[derive(Debug, Clone, IntoValue, FromValue, Serialize, Deserialize)]
pub enum SeriesStyle {
    Line,
}

macro_rules! impl_min_max_fns {
    ($(($fn_name:ident, $compare_op:tt, $axis:tt)),*) => {
        $(
            pub fn $fn_name(&self) -> Option<f64> {
                let mut value = self.series.first()?.0.first()?.$axis;
                for series in self.series.iter() {
                    for (v, _) in series.0.clone() {
                        if v $compare_op value {
                            value = v;
                        }
                    }
                }
                Some(value)
            }
        )*
    };
}

impl Chart2dOptions {
    impl_min_max_fns!(
        (min_x_value, <, 0),
        (max_x_value, >, 0),
        (min_y_value, <, 1),
        (max_y_value, >, 1)
    );

    pub fn x_range(&self) -> Option<(f64, f64)> {
        match self.x_range {
            Some(range) => Some(range),
            None => {
                let min = self.min_x_value()?;
                let max = self.max_x_value()?;
                Some((min, max))
            }
        }
    }

    pub fn y_range(&self) -> Option<(f64, f64)> {
        match self.y_range {
            Some(range) => Some(range),
            None => {
                let min = self.min_y_value()?;
                let max = self.max_y_value()?;
                Some((min, max))
            }
        }
    }

    // TODO: check if this could be self-consuming
    pub fn draw(&self, chart_builder: &mut ChartBuilder<SVGBackend>) {
        let (x_min, x_max) = self.x_range().unwrap();
        let (y_min, y_max) = self.y_range().unwrap();
        let x_spec = x_min..x_max;
        let y_spec = y_min..y_max;
        let mut ctx = chart_builder.build_cartesian_2d(x_spec, y_spec).unwrap();
        for (series, style) in self.series.iter() {
            match style {
                SeriesStyle::Line => {
                    ctx.draw_series(LineSeries::new(series.clone(), plotters::style::RED))
                        .unwrap();
                }
            }
        }
        ctx.configure_mesh().draw().unwrap();
    }
}

super::impl_custom_value!(Chart2dOptions);
