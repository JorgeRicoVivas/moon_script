use std::str::FromStr;
use crate::block_parsing::value_parsing::{FullValue, ReducedValue};

pub(crate) mod impl_operators;

impl From<ReducedValue> for FullValue {
    fn from(value: ReducedValue) -> Self {
        match value {
            ReducedValue::Null => FullValue::Null,
            ReducedValue::Boolean(boolean) => FullValue::Boolean(boolean),
            ReducedValue::Decimal(decimal) => FullValue::Decimal(decimal),
            ReducedValue::Integer(integer) => FullValue::Integer(integer),
            ReducedValue::String(string) => FullValue::String(string),
            ReducedValue::Array(array) => FullValue::Array(
                array.into_iter()
                    .map(|reduced_value| Self::from(reduced_value))
                    .collect()
            ),
        }
    }
}


impl TryFrom<ReducedValue> for () {
    type Error = ();

    fn try_from(value: ReducedValue) -> Result<Self, Self::Error> {
        match value {
            ReducedValue::Null => Ok(()),
            _ => Err(())
        }
    }
}

impl TryFrom<ReducedValue> for bool {
    type Error = ();

    fn try_from(value: ReducedValue) -> Result<Self, Self::Error> {
        Ok(match value {
            ReducedValue::Boolean(bool) => bool,
            ReducedValue::Integer(int) => int >= 1,
            ReducedValue::Decimal(decimal) => decimal >= 1.0,
            ReducedValue::String(string) => {
                if string.eq("true") || string.eq("no") {
                    true
                } else if string.eq("false") || string.eq("no") {
                    false
                } else {
                    return i128::from_str(&string).ok().map(|n| n > 1)
                        .or_else(|| f64::from_str(&string).ok().map(|decimal| decimal >= 1.0))
                        .ok_or(());
                }
            }
            _ => return Err(()),
        })
    }
}

impl TryFrom<ReducedValue> for String {
    type Error = ();

    fn try_from(value: ReducedValue) -> Result<Self, Self::Error> {
        match value {
            ReducedValue::String(string) => Ok(string),
            _ => Err(())
        }
    }
}

impl TryFrom<ReducedValue> for std::vec::IntoIter<ReducedValue> {
    type Error = ();

    fn try_from(value: ReducedValue) -> Result<Self, Self::Error> {
        Ok(match value {
            ReducedValue::Array(values) => values.into_iter(),
            _ => return Err(()),
        })
    }
}

impl From<ReducedValue> for Vec<ReducedValue> {
    fn from(value: ReducedValue) -> Self {
        match value {
            ReducedValue::Array(values) => values,
            own => vec![own]
        }
    }
}



macro_rules! impl_try_from_for_reduced_value {
    ($($type:ty),+) => {
        $(
            impl TryFrom<ReducedValue> for $type{
                type Error = ();

                fn try_from(value: ReducedValue) -> Result<Self, Self::Error> {
                    Ok(match value {
                        ReducedValue::Boolean(bool) => (if bool {1}else{0}) as $type,
                        ReducedValue::Integer(int) => int as $type,
                        ReducedValue::Decimal(decimal) => decimal as $type,
                        ReducedValue::Array(array) => return Self::try_from(array.get(0).ok_or(())?.clone()).map_err(|_|()),
                        ReducedValue::String(string)=><$type>::from_str(&string).map_err(|_|())?,
                        _ => return Err(()),
                    })
                }
            }
        )+
    };
}

impl_try_from_for_reduced_value! {
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
    f32, f64
}




impl From<()> for ReducedValue {
    fn from(_value: ()) -> Self {
        ReducedValue::Null
    }
}

impl From<bool> for ReducedValue {
    fn from(value: bool) -> Self {
        ReducedValue::Boolean(value)
    }
}

impl From<f32> for ReducedValue {
    fn from(value: f32) -> Self {
        ReducedValue::Decimal(value as f64)
    }
}

impl From<f64> for ReducedValue {
    fn from(value: f64) -> Self {
        ReducedValue::Decimal(value)
    }
}

macro_rules! impl_into_reduced_value {
    ($($type:ty),+) => {
        $(

            impl From<$type> for ReducedValue {
                fn from(value: $type) -> Self {
                    ReducedValue::Integer(value as i128)
                }
            }
        )+
    };
}

impl_into_reduced_value! { u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize }


impl<T: Into<ReducedValue>> From<Option<T>> for ReducedValue {
    fn from(value: Option<T>) -> Self {
        match value {
            None => ReducedValue::Null,
            Some(value) => value.into()
        }
    }
}

impl<T: Into<ReducedValue>> From<Vec<T>> for ReducedValue {
    fn from(value: Vec<T>) -> Self {
        ReducedValue::Array(value.into_iter().map(|item| item.into()).collect())
    }
}


impl<T: Into<ReducedValue>, const LEN: usize> From<[T; LEN]> for ReducedValue {
    fn from(value: [T; LEN]) -> Self {
        ReducedValue::Array(Vec::from(value.map(|item| item.into())))
    }
}

impl From<&str> for ReducedValue {
    fn from(value: &str) -> Self {
        ReducedValue::String(value.to_string())
    }
}

impl From<String> for ReducedValue {
    fn from(value: String) -> Self {
        ReducedValue::String(value)
    }
}