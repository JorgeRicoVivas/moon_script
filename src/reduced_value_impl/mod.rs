use crate::value_parsing::{FullValue, ReducedValue};

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

impl TryInto<()> for ReducedValue {
    type Error = ();

    fn try_into(self) -> Result<(), Self::Error> {
        match self {
            ReducedValue::Null => Ok(()),
            _ => Err(())
        }
    }
}

impl TryInto<bool> for ReducedValue {
    type Error = ();

    fn try_into(self) -> Result<bool, Self::Error> {
        Ok(match self {
            ReducedValue::Boolean(bool) => bool,
            ReducedValue::Integer(int) => int >= 1,
            ReducedValue::Decimal(decimal) => decimal >= 1.0,
            _ => return Err(()),
        })
    }
}

impl TryInto<std::vec::IntoIter<ReducedValue>> for ReducedValue {
    type Error = ();

    fn try_into(self) -> Result<std::vec::IntoIter<ReducedValue>, Self::Error> {
        Ok(match self {
            ReducedValue::Array(values) => values.into_iter(),
            _ => return Err(()),
        })
    }
}

impl TryInto<Vec<ReducedValue>> for ReducedValue {
    type Error = ();

    fn try_into(self) -> Result<Vec<ReducedValue>, Self::Error> {
        Ok(match self {
            ReducedValue::Array(values) => values,
            _ => return Err(()),
        })
    }
}

macro_rules! impl_try_into_for_reduced_value {
    ($($type:ty),+) => {
        $(
            impl TryInto<$type> for ReducedValue {
                type Error = ();

                fn try_into(self) -> Result<$type, Self::Error> {
                    Ok(match self {
                        ReducedValue::Boolean(bool) => (if bool {1}else{0}) as $type,
                        ReducedValue::Integer(int) => int as $type,
                        ReducedValue::Decimal(decimal) => decimal as $type,
                        _ => return Err(()),
                    })
                }
            }
        )+
    };
}

impl_try_into_for_reduced_value! {
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
    f32, f64
}