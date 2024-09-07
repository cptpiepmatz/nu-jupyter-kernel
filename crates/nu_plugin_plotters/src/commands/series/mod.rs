use nu_protocol::{FromValue, ShellError, Value};

use crate::value::{Coord1d, Coord2d};

mod line;
pub use line::*;

mod bar;
pub use bar::*;

fn series_from_value(input: Value) -> Result<Vec<Coord2d>, ShellError> {
    let input_span = input.span();
    let input = input.into_list()?;
    let first = input
        .first()
        .ok_or_else(|| ShellError::PipelineEmpty {
            dst_span: input_span,
        })?
        .clone();

    let number = Coord1d::from_value(first.clone());
    let tuple = <(Coord1d, Coord1d)>::from_value(first.clone());
    let coord = Coord2d::from_value(first);

    let mut series: Vec<Coord2d> = Vec::with_capacity(input.len());
    match (number, tuple, coord) {
        (Ok(_), _, _) => {
            // input: list<number>
            for (i, val) in input.into_iter().enumerate() {
                let val = Coord1d::from_value(val)?;
                let val = Coord2d {
                    x: Coord1d::from_int(i as i64),
                    y: val,
                };
                series.push(val);
            }
        }

        (_, Ok(_), _) => {
            // input: list<list<number>>
            for val in input {
                let (x, y) = <(Coord1d, Coord1d)>::from_value(val)?;
                let val = Coord2d { x, y };
                series.push(val);
            }
        }

        (_, _, Ok(_)) => {
            // input: list<record<x: number, y: number>>
            for val in input {
                let val = Coord2d::from_value(val)?;
                series.push(val);
            }
        }

        (Err(_), Err(_), Err(_)) => todo!("throw explaining error"),
    }

    Ok(series)
}
