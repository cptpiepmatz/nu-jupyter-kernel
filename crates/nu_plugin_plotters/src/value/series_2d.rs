use std::any::Any;

use nu_protocol::{CustomValue, FromValue, IntoValue, ShellError, Span, Type, Value};
use serde::{Deserialize, Serialize};

use super::color::Color;

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

impl Series2d {
    pub fn ty() -> Type {
        Type::Custom("plotters::series-2d".to_string().into_boxed_str())
    }
}
