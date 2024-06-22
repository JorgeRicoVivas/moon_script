use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::{Display, Formatter};

use crate::engine::context::ContextBuilder;
use crate::execution::ASTFunction;

#[derive(Clone, PartialEq, Debug)]
pub enum VBValue {
    Null,
    Boolean(bool),
    Integer(i128),
    Decimal(f64),
    String(String),
    Array(Vec<VBValue>),
}

impl VBValue {
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean(_) => "bool",
            Self::Integer(_) => "int",
            Self::Decimal(_) => "decimal",
            Self::String(_) => "string",
            Self::Array(_) => "array",
        }
    }
}

impl TryFrom<FullValue> for VBValue {
    type Error = ();
    fn try_from(value: FullValue) -> Result<Self, Self::Error> {
        Ok(match value {
            FullValue::Null => { VBValue::Null }
            FullValue::Boolean(v) => { VBValue::Boolean(v) }
            FullValue::Integer(v) => { VBValue::Integer(v) }
            FullValue::Decimal(v) => { VBValue::Decimal(v) }
            FullValue::String(v) => { VBValue::String(v) }
            FullValue::Array(v) => {
                let mut values = Vec::with_capacity(v.len());
                for value in v {
                    values.push(VBValue::try_from(value)?)
                };
                VBValue::Array(values)
            }
            _ => { return Err(()); }
        })
    }
}

impl Display for VBValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            VBValue::Null => f.write_str("null"),
            VBValue::Boolean(bool) => f.write_str(&*bool.to_string()),
            VBValue::Integer(int) => f.write_str(&*int.to_string()),
            VBValue::Decimal(dec) => f.write_str(&*dec.to_string()),
            VBValue::String(string) => f.write_str(&format!("\"{string}\"")),
            VBValue::Array(array) => {
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
pub enum FullValue {
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
            FullValue::Null => "null",
            FullValue::Boolean(_) => "bool",
            FullValue::Integer(_) => "int",
            FullValue::Decimal(_) => "decimal",
            FullValue::String(_) => "string",
            FullValue::Array(_) => "array",
            FullValue::Function(_) => "function",
            FullValue::Variable { block_level, var_index } => {
                return (context_builder
                    .get_variable_at(*block_level, *var_index).unwrap())
                    .inlineable_value()
                    .map(|know_value| know_value.type_name(context_builder))
                    .flatten();
            }
            FullValue::DirectVariable(_) => { unreachable!() }
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

    pub(crate) fn resolve_value_no_context(self) -> VBValue {
        match self {
            FullValue::Null => VBValue::Null,
            FullValue::Boolean(bool) => VBValue::Boolean(bool),
            FullValue::Decimal(decimal) => VBValue::Decimal(decimal),
            FullValue::Integer(integer) => VBValue::Integer(integer),
            FullValue::String(string) => VBValue::String(string),
            FullValue::Array(value) => VBValue::Array(value.into_iter()
                .map(|value| value.resolve_value_no_context())
                .collect()),
            _ => panic!()
        }
    }
}
