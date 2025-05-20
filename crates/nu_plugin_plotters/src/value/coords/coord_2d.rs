use chrono::{DateTime, FixedOffset};
use nu_protocol::{FromValue, IntoValue, ShellError, Span, Value};
use serde::{Deserialize, Serialize};

use super::Coord1d;

#[derive(Debug, Clone, Copy, IntoValue, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coord2d {
    pub x: XAxis,
    pub y: YAxis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XAxis {
    Coord(Coord1d),
    Date(DateTime<FixedOffset>),
}

impl IntoValue for XAxis {
    fn into_value(self, span: Span) -> Value {
        match self {
            XAxis::Coord(coord1d) => coord1d.into_value(span),
            XAxis::Date(date) => Value::date(date, span),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct YAxis(pub Coord1d);

impl IntoValue for YAxis {
    fn into_value(self, span: Span) -> Value {
        self.0.into_value(span)
    }
}

impl FromValue for Coord2d {
    fn from_value(v: Value) -> Result<Self, ShellError> {
        let tuple_date = Self::try_from_tuple_date(&v);
        let tuple = Self::try_from_tuple(&v);
        let record_date = Self::try_from_record_date(&v);
        let record = Self::try_from_record(&v);

        match (tuple_date, tuple, record_date, record) {
            (Ok(c), _, _, _) => Ok(c),
            (_, Ok(c), _, _) => Ok(c),
            (_, _, Ok(c), _) => Ok(c),
            (_, _, _, Ok(c)) => Ok(c),
            (Err(e0), Err(e1), Err(e2), Err(e3)) => Err(ShellError::GenericError {
                error: "Invalid 2D coordinate.".to_string(),
                msg: "A 2D coordinate needs to be either pair or a x-y-record of numbers."
                    .to_string(),
                span: None,
                help: Some("The x-axis also supports dates.".into()),
                inner: vec![e0, e1, e2, e3],
            })

        }
    }
}

impl Coord2d {
    fn try_from_tuple(v: &Value) -> Result<Self, ShellError> {
        let tuple = <(Coord1d, Coord1d)>::from_value(v.clone())?;
        Ok(Self {
            x: XAxis::Coord(tuple.0),
            y: YAxis(tuple.1)
        })
    }

    fn try_from_tuple_date(v: &Value) -> Result<Self, ShellError> {
        let tuple = <(DateTime<FixedOffset>, Coord1d)>::from_value(v.clone())?;
        Ok(Self {
            x: XAxis::Date(tuple.0),
            y: YAxis(tuple.1)
        })
    }

    fn try_from_record(v: &Value) -> Result<Self, ShellError> {
        #[derive(FromValue)]
        struct DTO {
            x: Coord1d,
            y: Coord1d,
        }

        let record = DTO::from_value(v.clone())?;
        Ok(Self {
            x: XAxis::Coord(record.x),
            y: YAxis(record.y)
        })
    }

    fn try_from_record_date(v: &Value) -> Result<Self, ShellError> {
        #[derive(FromValue)]
        struct DTO {
            x: DateTime<FixedOffset>,
            y: Coord1d,
        }

        let record = DTO::from_value(v.clone())?;
        Ok(Self {
            x: XAxis::Date(record.x),
            y: YAxis(record.y)
        })
    }
}
