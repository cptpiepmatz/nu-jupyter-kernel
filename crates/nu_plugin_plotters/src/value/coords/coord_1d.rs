use std::{cmp, ops::Sub};
use std::ops::{Add, AddAssign, Div};

use nu_protocol::{FromValue, IntoValue, ShellError, Span, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Coord1d {
    Int(i64),
    Float(f64), // we ensure that this is valid number
}

#[derive(Debug, Clone, Copy)]
pub enum Coord1dFromFloatError {
    Nan,
    Infinity,
}

impl Coord1d {
    pub fn from_int(int: i64) -> Self {
        Coord1d::Int(int)
    }

    pub fn from_float(float: f64) -> Result<Self, Coord1dFromFloatError> {
        match float.is_finite() {
            true => Ok(Self::Float(float)),
            false => match float.is_nan() {
                true => Err(Coord1dFromFloatError::Nan),
                false => Err(Coord1dFromFloatError::Infinity),
            },
        }
    }

    pub fn as_float(self) -> f64 {
        match self {
            Coord1d::Int(int) => int as f64,
            Coord1d::Float(float) => float,
        }
    }

    pub fn floor(self) -> i64 {
        match self {
            Coord1d::Int(int) => int,
            Coord1d::Float(float) => {
                let float = float.floor();
                debug_assert!(
                    float < i64::MAX as f64,
                    "Coord1d::Float was larger than i64::MAX"
                );
                debug_assert!(
                    float > i64::MIN as f64,
                    "Coord1d::Float was smaller than i64::MIN"
                );
                float as i64
            }
        }
    }

    pub fn round(self) -> i64 {
        match self {
            Coord1d::Int(int) => int,
            Coord1d::Float(float) => {
                let float = float.round();
                debug_assert!(
                    float < i64::MAX as f64,
                    "Coord1d::Float was larger than i64::MAX"
                );
                debug_assert!(
                    float > i64::MIN as f64,
                    "Coord1d::Float was smaller than i64::MIN"
                );
                float as i64
            }
        }
    }

    pub fn ceil(self) -> i64 {
        match self {
            Coord1d::Int(int) => int,
            Coord1d::Float(float) => {
                let float = float.ceil();
                debug_assert!(
                    float < i64::MAX as f64,
                    "Coord1d::Float was larger than i64::MAX"
                );
                debug_assert!(
                    float > i64::MIN as f64,
                    "Coord1d::Float was smaller than i64::MIN"
                );
                float as i64
            }
        }
    }
}

impl Eq for Coord1d {}

impl PartialEq for Coord1d {
    fn eq(&self, other: &Self) -> bool {
        let this = (*self).as_float();
        let that = (*other).as_float();
        this == that
    }
}

impl Ord for Coord1d {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match self.partial_cmp(other) {
            Some(ord) => ord,
            // this case shouldn't happen as we ensure that the float is a finite number
            None => panic!("Coord1::Float was not a number"),
        }
    }
}

impl PartialOrd for Coord1d {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        let this = (*self).as_float();
        let that = (*other).as_float();
        cmp::PartialOrd::partial_cmp(&this, &that)
    }
}

impl IntoValue for Coord1d {
    fn into_value(self, span: Span) -> Value {
        match self {
            Coord1d::Int(int) => Value::int(int, span),
            Coord1d::Float(float) => Value::float(float, span),
        }
    }
}

impl FromValue for Coord1d {
    fn from_value(v: Value) -> Result<Self, ShellError> {
        match v {
            Value::Int { val, .. } => Ok(Self::Int(val)),
            Value::Float { val, internal_span } => Self::from_float(val).map_err(|e| {
                let error = match e {
                    Coord1dFromFloatError::Nan => "Number is not a number",
                    Coord1dFromFloatError::Infinity => "Number is not finite",
                }
                .to_string();

                ShellError::GenericError {
                    error,
                    msg: "Coordinates need to be a valid number.".to_string(),
                    span: Some(internal_span),
                    help: None,
                    inner: vec![],
                }
            }),
            _ => Err(ShellError::CantConvert {
                to_type: Self::expected_type().to_string(),
                from_type: v.get_type().to_string(),
                span: v.span(),
                help: None,
            }),
        }
    }
}

impl Add for Coord1d {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Coord1d::Int(lhs), Coord1d::Int(rhs)) => Coord1d::Int(lhs + rhs),
            (Coord1d::Int(lhs), Coord1d::Float(rhs)) => Coord1d::Float(lhs as f64 + rhs),
            (Coord1d::Float(lhs), Coord1d::Int(rhs)) => Coord1d::Float(lhs + rhs as f64),
            (Coord1d::Float(lhs), Coord1d::Float(rhs)) => Coord1d::Float(lhs + rhs),
        }
    }
}

impl AddAssign for Coord1d {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.add(rhs);
    }
}

impl Sub for Coord1d {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Coord1d::Int(lhs), Coord1d::Int(rhs)) => Coord1d::Int(lhs - rhs),
            (Coord1d::Int(lhs), Coord1d::Float(rhs)) => Coord1d::Float(lhs as f64 - rhs),
            (Coord1d::Float(lhs), Coord1d::Int(rhs)) => Coord1d::Float(lhs - rhs as f64),
            (Coord1d::Float(lhs), Coord1d::Float(rhs)) => Coord1d::Float(lhs - rhs),
        }
    }
}

impl Div for Coord1d {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        let lhs = self.as_float();
        let rhs = rhs.as_float();
        Coord1d::Float(lhs / rhs)
    }
}

impl Default for Coord1d {
    fn default() -> Self {
        Self::Int(0)
    }
}
