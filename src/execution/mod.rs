use ast::Statement;
use alloc::fmt::Debug;
use alloc::vec::Vec;

use crate::function::VBFunction;
use crate::parsing::value_parsing::FullValue;

pub mod optimized_ast;
pub mod ast;

#[derive(Clone, Debug)]
pub struct ASTFunction {
    pub(crate) function: VBFunction,
    pub(crate) args: Vec<FullValue>,
}

#[derive(Clone, Debug)]
pub(crate) struct ConditionalStatements {
    pub(crate) condition: FullValue,
    pub(crate) statements: Vec<Statement>,
}


#[derive(Debug, Clone)]
pub struct RuntimeVariable {
    pub(crate) value: FullValue,
}

impl From<FullValue> for RuntimeVariable {
    fn from(value: FullValue) -> Self {
        RuntimeVariable::new(value)
    }
}

impl RuntimeVariable {
    pub(crate) fn new<Value: Into<FullValue>>(value: Value) -> Self {
        Self { value: value.into() }
    }
}