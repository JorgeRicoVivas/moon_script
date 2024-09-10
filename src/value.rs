use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::{Display, Formatter};

use crate::engine::context::ContextBuilder;
use crate::execution::ASTFunction;
use crate::parsing::MoonValueKind;

/// Values used as input and outputs on scripts
#[derive(Clone, PartialEq, Debug)]
pub enum MoonValue {
    Null,
    Boolean(bool),
    Integer(i128),
    Decimal(f64),
    String(String),
    Array(Vec<MoonValue>),
}

impl TryFrom<FullValue> for MoonValue {
    type Error = ();
    fn try_from(value: FullValue) -> Result<Self, Self::Error> {
        Ok(match value {
            FullValue::Null => { MoonValue::Null }
            FullValue::Boolean(v) => { MoonValue::Boolean(v) }
            FullValue::Integer(v) => { MoonValue::Integer(v) }
            FullValue::Decimal(v) => { MoonValue::Decimal(v) }
            FullValue::String(v) => { MoonValue::String(v) }
            FullValue::Array(v) => {
                let mut values = Vec::with_capacity(v.len());
                for value in v {
                    values.push(MoonValue::try_from(value)?)
                };
                MoonValue::Array(values)
            }
            _ => { return Err(()); }
        })
    }
}

impl Display for MoonValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            MoonValue::Null => f.write_str("null"),
            MoonValue::Boolean(bool) => f.write_str(&*bool.to_string()),
            MoonValue::Integer(int) => f.write_str(&*int.to_string()),
            MoonValue::Decimal(dec) => f.write_str(&*dec.to_string()),
            MoonValue::String(string) => f.write_str(&format!("\"{string}\"")),
            MoonValue::Array(array) => {
                let mut result = String::new();
                result.push('[');
                let mut is_first_value = true;
                array.iter().for_each(|value| {
                    if is_first_value {
                        result.push_str(&format!("{value}"));
                        is_first_value = false;
                    } else {
                        result.push_str(&format!(", {value}"));
                    }
                });
                result.push(']');
                f.write_str(&*result)
            }
        }
    }
}


#[derive(Debug, Clone)]
pub(crate) enum FullValue {
    Null,
    Boolean(bool),
    Integer(i128),
    Decimal(f64),
    String(String),
    Array(Vec<FullValue>),
    Function(ASTFunction),
    Variable { block_level: usize, var_index: usize },
    DirectVariable(usize),
}

impl PartialEq for FullValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Boolean(bool_1), Self::Boolean(bool_2)) => bool_1.eq(bool_2),
            (Self::Integer(int_1), Self::Integer(int_2)) => int_1.eq(int_2),
            (Self::Decimal(decimal_1), Self::Decimal(decimal_2)) => decimal_1.eq(decimal_2),
            (Self::String(string_1), Self::String(string_2)) => string_1.eq(string_2),
            (Self::Array(values_1), Self::Array(values_2)) => values_1.eq(values_2),
            (Self::Variable { block_level: block_level_1, var_index: var_index_1 },
                Self::Variable { block_level: block_level_2, var_index: var_index_2 })
            => block_level_1.eq(block_level_2) && var_index_1.eq(var_index_2),
            (Self::DirectVariable(variable_pos_1), Self::DirectVariable(variable_pos_2)) => variable_pos_1.eq(variable_pos_2),
            _ => false,
        }
    }
}


impl FullValue {
    pub(crate) fn is_constant_boolean_true(&self) -> bool {
        match self {
            FullValue::Boolean(bool) => { *bool }
            _ => { false }
        }
    }

    pub(crate) fn is_constant_boolean_false(&self) -> bool {
        match self {
            FullValue::Boolean(bool) => { !*bool }
            _ => { false }
        }
    }

    pub(crate) fn type_name(&self, context_builder: &mut ContextBuilder) -> Option<String> {
        Some(match self {
            Self::Null => MoonValueKind::Null.get_moon_value_type().unwrap(),
            Self::Boolean(_) => MoonValueKind::Boolean.get_moon_value_type().unwrap(),
            Self::Integer(_) => MoonValueKind::Integer.get_moon_value_type().unwrap(),
            Self::Decimal(_) => MoonValueKind::Decimal.get_moon_value_type().unwrap(),
            Self::String(_) => MoonValueKind::String.get_moon_value_type().unwrap(),
            Self::Array(_) => MoonValueKind::Array.get_moon_value_type().unwrap(),
            Self::Function(_) => MoonValueKind::Function.get_moon_value_type().unwrap(),
            Self::Variable { block_level, var_index } => {
                return (context_builder
                    .get_variable_at(*block_level, *var_index).unwrap())
                    .inlineable_value()
                    .map(|know_value| know_value.type_name(context_builder))
                    .flatten();
            }
            Self::DirectVariable(_) => { unreachable!() }
        }).map(|type_name| type_name.to_string())
    }

    pub(crate) fn is_simple_value(&self) -> bool {
        match self {
            FullValue::Null | FullValue::Boolean(_) | FullValue::Decimal(_) |
            FullValue::Integer(_) | FullValue::String(_) => true,
            FullValue::Array(values) => values.iter().all(|value| value.is_simple_value()),
            _ => false
        }
    }

    pub(crate) fn resolve_value_no_context(self) -> MoonValue {
        match self {
            FullValue::Null => MoonValue::Null,
            FullValue::Boolean(bool) => MoonValue::Boolean(bool),
            FullValue::Decimal(decimal) => MoonValue::Decimal(decimal),
            FullValue::Integer(integer) => MoonValue::Integer(integer),
            FullValue::String(string) => MoonValue::String(string),
            FullValue::Array(value) => MoonValue::Array(value.into_iter()
                .map(|value| value.resolve_value_no_context())
                .collect()),
            _ => panic!()
        }
    }
}
