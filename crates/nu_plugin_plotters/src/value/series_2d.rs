use std::any::Any;
use std::{cmp, iter};

use nu_protocol::{CustomValue, FromValue, IntoValue, Record, ShellError, Span, Type, Value};
use serde::{Deserialize, Serialize};

use super::color::Color;
use super::{Coord1d, Coord2d, Range};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Series2d {
    Line(Line2dSeries),
    Bar(Bar2dSeries),
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoValue)]
pub struct Line2dSeries {
    pub series: Vec<Coord2d>,
    pub color: Color,
    pub filled: bool,
    pub stroke_width: u32,
    pub point_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoValue)]
pub struct Bar2dSeries {
    pub series: Vec<Coord2d>,
    pub color: Color,
    pub filled: bool,
    pub stroke_width: u32,
}

impl Series2d {
    fn into_base_value(self, span: Span) -> Value {
        let kind = match &self {
            Series2d::Line(_) => "line",
            Series2d::Bar(_) => "bar",
        };
        let kind = ("kind".to_string(), Value::string(kind, span));

        let record = match self {
            Series2d::Line(line) => line.into_value(span),
            Series2d::Bar(bar) => bar.into_value(span),
        };
        let record = record
            .into_record()
            .expect("structs derive IntoValue via Value::Record");

        let iter = iter::once(kind).chain(record.into_iter());
        Value::record(Record::from_iter(iter), span)
    }
}

impl IntoValue for Series2d {
    fn into_value(self, span: Span) -> Value {
        Value::custom(Box::new(self), span)
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
        Ok(Series2d::into_base_value(self.clone(), span))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn Any {
        self
    }
}

impl Series2d {
    pub fn series(&self) -> &[Coord2d] {
        match self {
            Series2d::Line(line) => &line.series,
            Series2d::Bar(bar) => &bar.series,
        }
    }

    // FIXME: maybe we need to rethink this range function
    fn range<A, M>(&self, axis: A, map: M) -> Option<Range>
    where
        A: Fn(&Coord2d) -> Coord1d,
        M: Fn(Coord1d) -> (Coord1d, Coord1d),
    {
        let first = self.series().first()?;
        let (mut min, mut max) = (axis(first), axis(first));
        for (lower, upper) in self.series().iter().map(axis).map(map) {
            if lower < min {
                min = lower;
            }

            if upper > max {
                max = upper;
            }
        }

        Some(Range {
            min,
            max,
            metadata: None,
        })
    }

    pub fn x_range(&self) -> Option<Range> {
        self.range(
            |c| c.x,
            |c| match self {
                Series2d::Line(_) => (c, c),
                Series2d::Bar(_) => (c - Coord1d::Float(0.5), c + Coord1d::Float(0.5)),
            },
        )
    }

    pub fn y_range(&self) -> Option<Range> {
        self.range(
            |c| c.y,
            |c| match self {
                Series2d::Line(_) => (c, c),
                Series2d::Bar(_) => (cmp::min(Coord1d::Int(0), c), cmp::max(Coord1d::Int(0), c)),
            },
        )
    }

    pub fn ty() -> Type {
        Type::Custom("plotters::series-2d".to_string().into_boxed_str())
    }
}
