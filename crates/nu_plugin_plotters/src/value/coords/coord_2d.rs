use nu_protocol::{FromValue, IntoValue, ShellError, Value};
use serde::{Deserialize, Serialize};

use super::Coord1d;

#[derive(Debug, Clone, Copy, IntoValue, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coord2d {
    pub x: Coord1d,
    pub y: Coord1d,
}

impl FromValue for Coord2d {
    fn from_value(v: Value) -> Result<Self, ShellError> {
        #[derive(FromValue)]
        struct Coord2dDTO {
            x: Coord1d,
            y: Coord1d,
        }

        let tuple = <(Coord1d, Coord1d)>::from_value(v.clone());
        let record = Coord2dDTO::from_value(v);
        match (tuple, record) {
            (Ok((x, y)), _) => Ok(Self { x, y }),
            (_, Ok(Coord2dDTO { x, y })) => Ok(Self { x, y }),
            (Err(tuple_e), Err(record_e)) => Err(ShellError::GenericError {
                error: "Invalid 2D coordinate.".to_string(),
                msg: "A 2D coordinate needs to be either pair or a x-y-record of numbers."
                    .to_string(),
                span: None,
                help: None,
                inner: vec![tuple_e, record_e],
            }),
        }
    }
}
