use plotters::chart::{ChartBuilder, LabelAreaPosition};
use plotters::coord::Shift;
use plotters::element::PointCollection;
use plotters::prelude::{DrawingArea, DrawingBackend, Rectangle};
use plotters::series::{Histogram, LineSeries};
use plotters::style::{RGBAColor, ShapeStyle, BLACK};

use crate::value::{self, Coord2d};

mod svg;
pub use svg::*;

mod terminal;
pub use terminal::*;

fn draw<DB: DrawingBackend>(chart: value::Chart2d, drawing_area: DrawingArea<DB, Shift>) {
    if chart.series.is_empty() {
        todo!("return some error that empty series do not work")
    }

    // TODO: make better error
    let x_range = chart.x_range().unwrap();
    let y_range = chart.y_range().unwrap();

    if let Some(color) = chart.background {
        let color: RGBAColor = color.into();
        drawing_area.fill(&color).unwrap();
    }

    let mut chart_builder = ChartBuilder::on(&drawing_area);

    let [top, right, bottom, left] = chart.margin;
    chart_builder.margin_top(top);
    chart_builder.margin_right(right);
    chart_builder.margin_bottom(bottom);
    chart_builder.margin_left(left);

    let [top, right, bottom, left] = chart.label_area;
    chart_builder.set_label_area_size(LabelAreaPosition::Top, top);
    chart_builder.set_label_area_size(LabelAreaPosition::Right, right);
    chart_builder.set_label_area_size(LabelAreaPosition::Bottom, bottom);
    chart_builder.set_label_area_size(LabelAreaPosition::Left, left);

    if let Some(caption) = chart.caption {
        chart_builder.caption(caption, &BLACK);
    }

    let mut chart_context = chart_builder.build_cartesian_2d(x_range, y_range).unwrap();

    chart_context.configure_mesh().draw().unwrap();
    for value::Series2d {
        series,
        style,
        color,
        filled,
        stroke_width,
    } in chart.series
    {
        type S2S = value::Series2dStyle;
        match style {
            S2S::Line { point_size } => {
                let series = LineSeries::new(
                    series.into_iter().map(|value::Coord2d { x, y }| (x, y)),
                    ShapeStyle {
                        color: color.into(),
                        filled,
                        stroke_width,
                    },
                )
                .point_size(point_size);
                chart_context.draw_series(series).unwrap();
            }
            S2S::Bar { horizontal: false } => {
                let shape_style = ShapeStyle {
                    color: color.into(),
                    filled,
                    stroke_width,
                };
                let histogram = Histogram::vertical(&chart_context)
                    .data(series.into_iter().map(|Coord2d { x, y }| (x, y)));
                // TODO: pull this shifting in a separate fn
                chart_context.draw_series(histogram.into_iter().map(|rect| {
                    let mut points = rect.point_iter().iter().cloned();
                    let first = points.next().expect("first corner");
                    let second = points.next().expect("second corner");
                    let offset = (second.0 - first.0) / value::Coord1d::Int(2);

                    let first = (first.0 - offset, first.1);
                    let second = (second.0 - offset, second.1);

                    let rect = Rectangle::new([first, second], shape_style);
                    // TODO: set margin
                    rect
                })).unwrap();
            }
            S2S::Bar { horizontal: true } => todo!(),
        }
    }
}
