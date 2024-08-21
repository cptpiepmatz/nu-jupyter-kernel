use std::any::Any;

use nu_protocol::{CustomValue, IntoValue, ShellError, Span, Type, Value};
use serde::{Deserialize, Serialize};

use super::color::Color;
use super::series_2d::Series2d;

#[derive(Debug, Clone, IntoValue, Serialize, Deserialize)]
pub struct Chart2d {
    pub series: Vec<Series2d>,
    pub width: u32,
    pub height: u32,
    pub background: Option<Color>,
    pub caption: Option<String>,
}

#[typetag::serde]
impl CustomValue for Chart2d {
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

impl Chart2d {
    pub fn ty() -> Type {
        Type::Custom("plotters::chart-2d".to_string().into_boxed_str())
    }
}
