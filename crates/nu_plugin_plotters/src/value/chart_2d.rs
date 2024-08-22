use std::any::Any;

use nu_protocol::{CustomValue, FromValue, IntoValue, ShellError, Span, Type, Value};
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

impl Default for Chart2d {
    fn default() -> Self {
        Self { 
            series: Vec::new(), 
            width: 600, 
            height: 400, 
            background: None, 
            caption: None 
        }
    }
}

impl FromValue for Chart2d {
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