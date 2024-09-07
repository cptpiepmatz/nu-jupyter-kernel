use std::ops::{self, Bound};

use nu_protocol::{FloatRange, FromValue, IntoValue, ShellError, Type, Value};
use plotters::coord::ranged1d::{KeyPointHint, NoDefaultFormatting, ValueFormatter};
use plotters::coord::types::{RangedCoordf64, RangedCoordi64};
use plotters::prelude::{DiscreteRanged, Ranged};
use serde::{Deserialize, Serialize};

use super::Coord1d;

// TODO: ensure that ranges are always min to max
#[derive(Debug, Clone, Copy, IntoValue, Serialize, Deserialize)]
pub struct Range {
    pub min: Coord1d,
    pub max: Coord1d,
    pub metadata: Option<RangeMetadata>,
}

#[derive(Debug, Clone, Copy, IntoValue, Serialize, Deserialize)]
pub struct RangeMetadata {
    pub discrete_key_points: bool,
}

impl FromValue for Range {
    fn from_value(v: Value) -> Result<Self, ShellError> {
        match v {
            Value::Range { val, internal_span } => {
                // TODO: try IntRange here first
                let range = FloatRange::from(*val);
                let min = range.start();
                let max = match range.end() {
                    Bound::Included(max) => max,
                    Bound::Excluded(max) => max,
                    Bound::Unbounded => {
                        return Err(ShellError::CantConvert {
                            to_type: Self::expected_type().to_string(),
                            from_type: Type::Range.to_string(),
                            span: internal_span,
                            help: Some("Try a bounded range instead.".to_string()),
                        })
                    }
                };

                let min = Coord1d::from_value(min.into_value(internal_span))?;
                let max = Coord1d::from_value(max.into_value(internal_span))?;

                Ok(Self {
                    min,
                    max,
                    metadata: None,
                })
            }

            v @ Value::List { .. } => {
                let [min, max] = <[Coord1d; 2]>::from_value(v)?;
                Ok(Self {
                    min,
                    max,
                    metadata: None,
                })
            }

            v @ Value::Record { .. } => {
                #[derive(Debug, FromValue)]
                struct RangeDTO {
                    min: Coord1d,
                    max: Coord1d,
                }

                let RangeDTO { min, max } = RangeDTO::from_value(v)?;
                Ok(Self {
                    min,
                    max,
                    metadata: None,
                })
            }

            v => Err(ShellError::CantConvert {
                to_type: Self::expected_type().to_string(),
                from_type: v.get_type().to_string(),
                span: v.span(),
                help: None,
            }),
        }
    }

    fn expected_type() -> Type {
        Type::List(Box::new(Coord1d::expected_type()))
    }
}

impl From<Range> for RangedCoordf64 {
    fn from(value: Range) -> Self {
        RangedCoordf64::from(value.min.as_float()..value.max.as_float())
    }
}

impl Ranged for Range {
    type FormatOption = NoDefaultFormatting;
    type ValueType = Coord1d;

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        RangedCoordf64::from(*self).map(&value.as_float(), limit)
    }

    fn key_points<Hint: KeyPointHint>(&self, hint: Hint) -> Vec<Self::ValueType> {
        if let Some(RangeMetadata {
            discrete_key_points: true,
        }) = self.metadata
        {
            let lower = self.min.ceil();
            let upper = self.max.floor();
            return (lower..=upper).map(Coord1d::Int).collect();
        }

        match (self.min, self.max) {
            (Coord1d::Int(_), Coord1d::Int(_)) => RangedCoordi64::from(*self)
                .key_points(hint)
                .into_iter()
                .map(Coord1d::from_int)
                .collect(),

            (Coord1d::Int(_), Coord1d::Float(_)) |
            (Coord1d::Float(_), Coord1d::Int(_)) |
            (Coord1d::Float(_), Coord1d::Float(_)) => {
                // TODO: check here if we want f64 key points and if from_float may expects None
                // values
                RangedCoordf64::from(*self)
                    .key_points(hint)
                    .into_iter()
                    .map(|float| Coord1d::from_float(float).unwrap())
                    .collect()
            }
        }
    }

    fn range(&self) -> ops::Range<Self::ValueType> {
        self.min..self.max
    }
}

impl From<Range> for RangedCoordi64 {
    fn from(value: Range) -> Self {
        RangedCoordi64::from(value.min.floor()..value.max.ceil())
    }
}

impl DiscreteRanged for Range {
    fn size(&self) -> usize {
        RangedCoordi64::from(*self).size()
    }

    fn index_of(&self, value: &Self::ValueType) -> Option<usize> {
        RangedCoordi64::from(*self).index_of(&value.round())
    }

    fn from_index(&self, index: usize) -> Option<Self::ValueType> {
        RangedCoordi64::from(*self)
            .from_index(index)
            .map(Coord1d::from_int)
    }
}

impl ValueFormatter<Coord1d> for Range {
    fn format(value: &Coord1d) -> String {
        match value {
            Coord1d::Int(value) => RangedCoordi64::format(value),
            Coord1d::Float(value) => RangedCoordf64::format(value),
        }
    }

    fn format_ext(&self, value: &Coord1d) -> String {
        Self::format(value)
    }
}
