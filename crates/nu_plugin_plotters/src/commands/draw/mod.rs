use plotters::chart::{ChartBuilder, LabelAreaPosition};
use plotters::coord::Shift;
use plotters::prelude::{DrawingArea, DrawingBackend};
use plotters::series::LineSeries;
use plotters::style::{RGBAColor, ShapeStyle, BLACK};

use crate::value;

mod svg;
pub use svg::*;

fn draw<DB: DrawingBackend>(chart: value::Chart2d, drawing_area: DrawingArea<DB, Shift>) {
    if chart.series.is_empty() {
        todo!("return some error that empty series do not work")
    }

    let x_spec = chart
        .x_range()
        .map(|(min, max)| min..max)
        .expect("not empty");
    let y_spec = chart
        .y_range()
        .map(|(min, max)| min..max)
        .expect("not empty");

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

    let mut chart_context = chart_builder.build_cartesian_2d(x_spec, y_spec).unwrap();

    chart_context.configure_mesh().draw().unwrap();
    for value::Series2d {
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
        }
    }
}
