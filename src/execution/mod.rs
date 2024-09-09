use alloc::fmt::Debug;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use simple_detailed_error::{SimpleErrorDetail, SimpleErrorExplanation};

use ast::Statement;

use crate::function::MoonFunction;
use crate::value::FullValue;

pub mod optimized_ast;
pub mod ast;

/// An error occurred when running a script
#[derive(Debug)]
pub enum RuntimeError {
    /// A function returned [Result::<_, String>::Err], being this is string an error message that
    /// is returned in 'function_error_message'.
    FunctionError { function_error_message: String },
    /// A predicate couldn't be calculated, this is the same as a [RuntimeError::FunctionError], but
    /// specific for 'if' and 'while' predicates.
    CannotTurnPredicateToBool { type_of_statement: &'static str, function_error_message: String },
    /// An argument to a function couldn't be parsed as a [crate::MoonValue].
    CannotParseArgument,
    /// A function tried to run, but an argument was missing.
    AnArgumentIsMissing,
}

impl RuntimeError {
    pub(crate) fn explain(&self) -> String {
        match self {
            RuntimeError::CannotTurnPredicateToBool { type_of_statement, function_error_message } =>
                format!("Could not parse predicate of a {type_of_statement} block due to: {function_error_message}"),
            RuntimeError::FunctionError { function_error_message } =>
                format!("Could execute a function due to: {function_error_message}"),
            RuntimeError::CannotParseArgument => "A function argument type is wrong".to_string(),
            RuntimeError::AnArgumentIsMissing => "A function is missing an argument".to_string(),
        }
    }
}

impl SimpleErrorDetail for RuntimeError {
    fn explain_error(&self) -> SimpleErrorExplanation {
        SimpleErrorExplanation::new().explanation(self.explain())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ASTFunction {
    pub(crate) function: MoonFunction,
    pub(crate) args: Vec<FullValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ConditionalStatements {
    pub(crate) condition: FullValue,
    pub(crate) statements: Vec<Statement>,
}


#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeVariable {
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