use plotters::chart::{ChartBuilder, ChartContext, LabelAreaPosition};
use plotters::coord::Shift;
use plotters::prelude::{Cartesian2d, DrawingArea, DrawingBackend, Rectangle};
use plotters::series::LineSeries;
use plotters::style::{RGBAColor, ShapeStyle, BLACK};

use crate::value::{
    self, Bar2dSeries, Coord1d, Coord2d, Line2dSeries, Range, RangeMetadata, Series2d,
};

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
        .any(|series| matches!(series, Series2d::Bar(_)))
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

    let mut mesh = chart_context.configure_mesh();
    if !chart.x_mesh {
        mesh.disable_x_mesh();
    };
    if !chart.y_mesh {
        mesh.disable_y_mesh();
    };
    mesh.draw().unwrap();

    for series in chart.series {
        match series {
            value::Series2d::Line(series) => draw_line(&mut chart_context, series),
            value::Series2d::Bar(series) => draw_vertical_bar(&mut chart_context, series),
        }
    }
}

fn draw_line<DB: DrawingBackend>(
    chart_context: &mut ChartContext<DB, Cartesian2d<Range, Range>>,
    series: Line2dSeries,
) {
    let Line2dSeries {
        series,
        color,
        filled,
        stroke_width,
        point_size,
    } = series;
    let shape_style = ShapeStyle {
        color: color.into(),
        filled,
        stroke_width,
    };
    let series = LineSeries::new(
        series.into_iter().map(|value::Coord2d { x, y }| (x, y)),
        shape_style,
    )
    .point_size(point_size);
    chart_context.draw_series(series).unwrap();
}

fn draw_vertical_bar<DB: DrawingBackend>(
    chart_context: &mut ChartContext<DB, Cartesian2d<Range, Range>>,
    series: Bar2dSeries,
) {
    let Bar2dSeries {
        series,
        color,
        filled,
        stroke_width,
    } = series;
    let shape_style = ShapeStyle {
        color: color.into(),
        filled,
        stroke_width,
    };
    let rect_iter = series.into_iter().map(|Coord2d { x, y }| {
        let half_width = Coord1d::Float(0.8) / Coord1d::Int(2);
        let value_point = (x - half_width, y);
        let base_point = (x + half_width, Coord1d::Int(0));
        Rectangle::new([value_point, base_point], shape_style)
    });
    chart_context.draw_series(rect_iter).unwrap();
}
