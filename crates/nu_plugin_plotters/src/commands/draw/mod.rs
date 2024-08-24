use plotters::chart::{ChartBuilder, ChartContext, LabelAreaPosition};
use plotters::coord::Shift;
use plotters::prelude::{Cartesian2d, DrawingArea, DrawingBackend, Rectangle};
use plotters::series::LineSeries;
use plotters::style::{RGBAColor, ShapeStyle, BLACK};

use crate::value::{self, Coord1d, Coord2d, Range, RangeMetadata, Series2dStyle};

mod svg;
pub use svg::*;

mod terminal;
pub use terminal::*;

fn draw<DB: DrawingBackend>(chart: value::Chart2d, drawing_area: DrawingArea<DB, Shift>) {
    if chart.series.is_empty() {
        todo!("return some error that empty series do not work")
    }

    // TODO: make better error
    let mut x_range = chart.x_range().unwrap();
    let y_range = chart.y_range().unwrap();

    // bar charts typically want to display all the discrete points
    if chart
        .series
        .iter()
        .any(|series| matches!(series.style, Series2dStyle::Bar { .. }))
    {
        x_range.metadata = Some(RangeMetadata {
            discrete_key_points: true,
        });
    }

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
        let shape_style = ShapeStyle {
            color: color.into(),
            filled,
            stroke_width,
        };

        type S2S = value::Series2dStyle;
        match style {
            S2S::Line { point_size } => {
                draw_line(&mut chart_context, series, shape_style, point_size)
            }
            S2S::Bar { horizontal: false } => {
                draw_vertical_bar(&mut chart_context, series, shape_style)
            }
            S2S::Bar { horizontal: true } => todo!(),
        }
    }
}

fn draw_line<DB: DrawingBackend>(
    chart_context: &mut ChartContext<DB, Cartesian2d<Range, Range>>,
    series: Vec<Coord2d>,
    shape_style: ShapeStyle,
    point_size: u32,
) {
    let series = LineSeries::new(
        series.into_iter().map(|value::Coord2d { x, y }| (x, y)),
        shape_style,
    )
    .point_size(point_size);
    chart_context.draw_series(series).unwrap();
}

fn draw_vertical_bar<DB: DrawingBackend>(
    chart_context: &mut ChartContext<DB, Cartesian2d<Range, Range>>,
    series: Vec<Coord2d>,
    shape_style: ShapeStyle,
) {
    let rect_iter = series.into_iter().map(|Coord2d { x, y }| {
        let half_width = Coord1d::Float(0.8) / Coord1d::Int(2);
        let value_point = (x - half_width, y);
        let base_point = (x + half_width, Coord1d::Int(0));
        Rectangle::new([value_point, base_point], shape_style)
    });
    chart_context.draw_series(rect_iter).unwrap();
}
