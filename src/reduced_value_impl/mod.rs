use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::str::FromStr;

use crate::value::{FullValue, MoonValue};

pub(crate) mod impl_operators;

impl From<MoonValue> for FullValue {
    fn from(value: MoonValue) -> Self {
        match value {
            MoonValue::Null => FullValue::Null,
            MoonValue::Boolean(boolean) => FullValue::Boolean(boolean),
            MoonValue::Decimal(decimal) => FullValue::Decimal(decimal),
            MoonValue::Integer(integer) => FullValue::Integer(integer),
            MoonValue::String(string) => FullValue::String(string),
            MoonValue::Array(array) => FullValue::Array(
                array.into_iter()
                    .map(|reduced_value| Self::from(reduced_value))
                    .collect()
            ),
        }
    }
}


impl TryFrom<MoonValue> for () {
    type Error = ();

    fn try_from(value: MoonValue) -> Result<Self, Self::Error> {
        match value {
            MoonValue::Null => Ok(()),
            _ => Err(())
        }
    }
}

impl TryFrom<MoonValue> for bool {
    type Error = ();

    fn try_from(value: MoonValue) -> Result<Self, Self::Error> {
        Ok(match value {
            MoonValue::Boolean(bool) => bool,
            MoonValue::Integer(int) => int >= 1,
            MoonValue::Decimal(decimal) => decimal >= 1.0,
            MoonValue::String(string) => {
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

impl TryFrom<MoonValue> for String {
    type Error = ();

    fn try_from(value: MoonValue) -> Result<Self, Self::Error> {
        match value {
            MoonValue::String(string) => Ok(string),
            _ => Err(())
        }
    }
}

impl<T: TryFrom<MoonValue>> TryFrom<MoonValue> for Vec<T> where T::Error: Default {
    type Error = T::Error;

    fn try_from(value: MoonValue) -> Result<Self, Self::Error> {
        Ok(match value {
            MoonValue::Null => Vec::new(),
            MoonValue::Array(values) => {
                let mut res = Vec::with_capacity(values.len());
                for value in values.into_iter() {
                    res.push(T::try_from(value)?);
                }
                res
            }
            other => vec![T::try_from(other)?]
        })
    }
}

impl TryFrom<MoonValue> for vec::IntoIter<MoonValue> {
    type Error = ();

    fn try_from(value: MoonValue) -> Result<Self, Self::Error> {
        Ok(match value {
            MoonValue::Array(values) => values.into_iter(),
            _ => return Err(()),
        })
    }
}





macro_rules! impl_try_from_for_reduced_value {
    ($($type:ty),+) => {
        $(
            impl TryFrom<MoonValue> for $type{
                type Error = ();

                fn try_from(value: MoonValue) -> Result<Self, Self::Error> {
                    Ok(match value {
                        MoonValue::Boolean(bool) => (if bool {1}else{0}) as $type,
                        MoonValue::Integer(int) => int as $type,
                        MoonValue::Decimal(decimal) => decimal as $type,
                        MoonValue::Array(array) => return Self::try_from(array.get(0).ok_or(())?.clone()).map_err(|_|()),
                        MoonValue::String(string)=><$type>::from_str(&string).map_err(|_|())?,
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




impl From<()> for MoonValue {
    fn from(_value: ()) -> Self {
        MoonValue::Null
    }
}

impl From<bool> for MoonValue {
    fn from(value: bool) -> Self {
        MoonValue::Boolean(value)
    }
}

impl From<f32> for MoonValue {
    fn from(value: f32) -> Self {
        MoonValue::Decimal(value as f64)
    }
}

impl From<f64> for MoonValue {
    fn from(value: f64) -> Self {
        MoonValue::Decimal(value)
    }
}

macro_rules! impl_into_reduced_value {
    ($($type:ty),+) => {
        $(

            impl From<$type> for MoonValue {
                fn from(value: $type) -> Self {
                    MoonValue::Integer(value as i128)
                }
            }
        )+
    };
}

impl_into_reduced_value! { u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize }


impl<T: Into<MoonValue>> From<Option<T>> for MoonValue {
    fn from(value: Option<T>) -> Self {
        match value {
            None => MoonValue::Null,
            Some(value) => value.into()
        }
    }
}

impl<T: Into<MoonValue>> From<Vec<T>> for MoonValue {
    fn from(value: Vec<T>) -> Self {
        MoonValue::Array(value.into_iter().map(|item| item.into()).collect())
    }
}


impl<T: Into<MoonValue>, const LEN: usize> From<[T; LEN]> for MoonValue {
    fn from(value: [T; LEN]) -> Self {
        MoonValue::Array(Vec::from(value.map(|item| item.into())))
    }
}

impl From<&str> for MoonValue {
    fn from(value: &str) -> Self {
        MoonValue::String(value.to_string())
    }
}

impl From<String> for MoonValue {
    fn from(value: String) -> Self {
        MoonValue::String(value)
    }
}