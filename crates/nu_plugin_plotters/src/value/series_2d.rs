use std::any::Any;

use nu_protocol::{CustomValue, FromValue, IntoValue, ShellError, Span, Type, Value};
use serde::{Deserialize, Serialize};

use super::{color::Color, Range};

#[derive(Debug, Clone, IntoValue, Serialize, Deserialize)]
pub struct Series2d {
    pub series: Vec<Coord2d>,
    pub style: Series2dStyle,
    pub color: Color,
    pub filled: bool,
    pub stroke_width: u32,
    pub point_size: u32,
}

#[derive(Debug, Clone, IntoValue, FromValue, Serialize, Deserialize)]
pub struct Coord2d {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, IntoValue, Serialize, Deserialize)]
pub enum Series2dStyle {
    Line,
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
        Ok(self.clone().into_value(span))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn Any {
        self
    }
}

macro_rules! xy_range {
    ($fn_name:ident: $xy:ident) => {
        pub fn $fn_name(&self) -> Option<Range> {
            let first = self.series.first()?;
            let (mut min, mut max) = (first.$xy, first.$xy);
            for $xy in self.series.iter().map(|c| c.$xy) {
                if $xy < min {
                    min = $xy
                }
                if $xy > max {
                    max = $xy
                }
            }

            Some(Range { min, max })
        }
    };
}

impl Series2d {
    xy_range!(x_range: x);

    xy_range!(y_range: y);

    pub fn ty() -> Type {
        Type::Custom("plotters::series-2d".to_string().into_boxed_str())
    }
}
