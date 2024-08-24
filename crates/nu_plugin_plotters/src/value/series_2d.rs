use std::any::Any;
use std::cmp;

use nu_protocol::{record, CustomValue, FromValue, IntoValue, ShellError, Span, Type, Value};
use serde::{Deserialize, Serialize};

use super::color::Color;
use super::{Coord1d, Coord2d, Range};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Series2d {
    pub series: Vec<Coord2d>,
    pub style: Series2dStyle,
    pub color: Color,
    pub filled: bool,
    pub stroke_width: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Series2dStyle {
    Line { point_size: u32 },
    Bar { horizontal: bool },
}

impl IntoValue for Series2d {
    fn into_value(self, span: Span) -> Value {
        let Series2d {
            series,
            style,
            color,
            filled,
            stroke_width,
        } = self;

        let mut record = record! {
            "series" => series.into_value(span),
            "color" => color.into_value(span),
            "filled" => filled.into_value(span),
            "stroke_width" => stroke_width.into_value(span),
        };

        match style {
            Series2dStyle::Line { point_size } => {
                record.push("style", "line".to_string().into_value(span));
                record.push("point_size", point_size.into_value(span));
            }
            Series2dStyle::Bar { horizontal } => {
                record.push("style", "bar".to_string().into_value(span));
                record.push("horizontal", horizontal.into_value(span));
            }
        }

        Value::record(record, span)
    }
}

impl FromValue for Series2d {
    fn from_value(v: Value) -> Result<Self, ShellError> {
        let span = v.span();
        let v = v.into_custom_value()?;
        match v.as_any().downcast_ref::<Self>() {
            Some(v) => Ok(v.clone()),
            None => {
                return Err(ShellError::CantConvert {
                    to_type: Self::ty().to_string(),
                    from_type: v.type_name(),
                    span,
                    help: None,
                })
            }
        }
    }

    fn expected_type() -> Type {
        Self::ty()
    }
}

#[typetag::serde]
impl CustomValue for Series2d {
    fn clone_value(&self, span: Span) -> Value {
        Value::custom(Box::new(self.clone()), span)
    }

    fn type_name(&self) -> String {
        Self::ty().to_string()
    }

    fn to_base_value(&self, span: Span) -> Result<Value, ShellError> {
        Ok(Series2d::into_value(self.clone(), span))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn Any {
        self
    }
}

impl Series2d {
    // FIXME: maybe we need to rethink this range function
    fn range<A, M>(&self, axis: A, map: M) -> Option<Range> where A: Fn(&Coord2d) -> Coord1d, M: Fn(Coord1d) -> (Coord1d, Coord1d) {
        let first = self.series.first()?;
        let (mut min, mut max) = (axis(first), axis(first));
        for (lower, upper) in self.series.iter().map(axis).map(map) {
            if lower < min {
                min = lower;
            }

            if upper > max {
                max = upper;
            }
        }

        Some(Range { min, max })
    }

    pub fn x_range(&self) -> Option<Range> {
        self.range(|c| c.x, |c| match self.style {
            Series2dStyle::Line { .. } => (c, c),
            Series2dStyle::Bar { .. } => (c - Coord1d::Float(0.6), c + Coord1d::Float(0.6)),
        })
    }

    pub fn y_range(&self) -> Option<Range> {
        self.range(|c| c.y, |c| match self.style {
            Series2dStyle::Line { .. } => (c, c),
            Series2dStyle::Bar { .. } => (cmp::min(Coord1d::Int(0), c), cmp::max(Coord1d::Int(0), c + Coord1d::Float(0.1))),
        })
    }

    pub fn ty() -> Type {
        Type::Custom("plotters::series-2d".to_string().into_boxed_str())
    }
}
