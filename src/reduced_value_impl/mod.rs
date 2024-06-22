use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::str::FromStr;

use crate::value::{FullValue, VBValue};

pub(crate) mod impl_operators;

impl From<VBValue> for FullValue {
    fn from(value: VBValue) -> Self {
        match value {
            VBValue::Null => FullValue::Null,
            VBValue::Boolean(boolean) => FullValue::Boolean(boolean),
            VBValue::Decimal(decimal) => FullValue::Decimal(decimal),
            VBValue::Integer(integer) => FullValue::Integer(integer),
            VBValue::String(string) => FullValue::String(string),
            VBValue::Array(array) => FullValue::Array(
                array.into_iter()
                    .map(|reduced_value| Self::from(reduced_value))
                    .collect()
            ),
        }
    }
}


impl TryFrom<VBValue> for () {
    type Error = ();

    fn try_from(value: VBValue) -> Result<Self, Self::Error> {
        match value {
            VBValue::Null => Ok(()),
            _ => Err(())
        }
    }
}

impl TryFrom<VBValue> for bool {
    type Error = ();

    fn try_from(value: VBValue) -> Result<Self, Self::Error> {
        Ok(match value {
            VBValue::Boolean(bool) => bool,
            VBValue::Integer(int) => int >= 1,
            VBValue::Decimal(decimal) => decimal >= 1.0,
            VBValue::String(string) => {
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

impl TryFrom<VBValue> for String {
    type Error = ();

    fn try_from(value: VBValue) -> Result<Self, Self::Error> {
        match value {
            VBValue::String(string) => Ok(string),
            _ => Err(())
        }
    }
}

impl<T: TryFrom<VBValue>> TryFrom<VBValue> for Vec<T> where T::Error: Default {
    type Error = T::Error;

    fn try_from(value: VBValue) -> Result<Self, Self::Error> {
        Ok(match value {
            VBValue::Null => Vec::new(),
            VBValue::Array(values) => {
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

impl TryFrom<VBValue> for vec::IntoIter<VBValue> {
    type Error = ();

    fn try_from(value: VBValue) -> Result<Self, Self::Error> {
        Ok(match value {
            VBValue::Array(values) => values.into_iter(),
            _ => return Err(()),
        })
    }
}





macro_rules! impl_try_from_for_reduced_value {
    ($($type:ty),+) => {
        $(
            impl TryFrom<VBValue> for $type{
                type Error = ();

                fn try_from(value: VBValue) -> Result<Self, Self::Error> {
                    Ok(match value {
                        VBValue::Boolean(bool) => (if bool {1}else{0}) as $type,
                        VBValue::Integer(int) => int as $type,
                        VBValue::Decimal(decimal) => decimal as $type,
                        VBValue::Array(array) => return Self::try_from(array.get(0).ok_or(())?.clone()).map_err(|_|()),
                        VBValue::String(string)=><$type>::from_str(&string).map_err(|_|())?,
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




impl From<()> for VBValue {
    fn from(_value: ()) -> Self {
        VBValue::Null
    }
}

impl From<bool> for VBValue {
    fn from(value: bool) -> Self {
        VBValue::Boolean(value)
    }
}

impl From<f32> for VBValue {
    fn from(value: f32) -> Self {
        VBValue::Decimal(value as f64)
    }
}

impl From<f64> for VBValue {
    fn from(value: f64) -> Self {
        VBValue::Decimal(value)
    }
}

macro_rules! impl_into_reduced_value {
    ($($type:ty),+) => {
        $(

            impl From<$type> for VBValue {
                fn from(value: $type) -> Self {
                    VBValue::Integer(value as i128)
                }
            }
        )+
    };
}

impl_into_reduced_value! { u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize }


impl<T: Into<VBValue>> From<Option<T>> for VBValue {
    fn from(value: Option<T>) -> Self {
        match value {
            None => VBValue::Null,
            Some(value) => value.into()
        }
    }
}

impl<T: Into<VBValue>> From<Vec<T>> for VBValue {
    fn from(value: Vec<T>) -> Self {
        VBValue::Array(value.into_iter().map(|item| item.into()).collect())
    }
}


impl<T: Into<VBValue>, const LEN: usize> From<[T; LEN]> for VBValue {
    fn from(value: [T; LEN]) -> Self {
        VBValue::Array(Vec::from(value.map(|item| item.into())))
    }
}

impl From<&str> for VBValue {
    fn from(value: &str) -> Self {
        VBValue::String(value.to_string())
    }
}

impl From<String> for VBValue {
    fn from(value: String) -> Self {
        VBValue::String(value)
    }
}