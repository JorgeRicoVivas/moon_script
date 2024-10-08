use alloc::fmt::Debug;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::mem;

use crate::execution::{ASTFunction, ConditionalStatements, RuntimeError, RuntimeVariable};
use crate::execution::optimized_ast::OptimizedAST;
use crate::HashMap;
use crate::value::{FullValue, MoonValue};

/// Compiled Script
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AST {
    pub(crate) statements: Vec<Statement>,
    pub(crate) variables: Vec<RuntimeVariable>,
    pub(crate) parameterized_variables: HashMap<String, usize>,
}

impl AST {

    /// Gets an executor for this script, from this executor you can both give input variables and
    /// execute it afterward.
    pub fn executor(&self) -> ASTExecutor<'_> {
        ASTExecutor::new(self)
    }

    /// Turns this AST into an [OptimizedAST], using [OptimizedAST] is always preferred over [AST].
    pub fn to_optimized_ast(self) -> OptimizedAST {
        OptimizedAST::from(self)
    }

    /// Executes the script withouth any input variables, if you want to specify them, get its
    /// [Self::executor] and push variables to it with [ASTExecutor::push_variable] before calling
    /// [ASTExecutor::execute].
    pub fn execute(&self) -> Result<MoonValue, RuntimeError> {
        self.executor().execute()
    }

}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Statement {
    WhileBlock { condition: FullValue, statements: Vec<Statement> },
    IfElseBlock { conditional_statements: Vec<ConditionalStatements> },
    UnoptimizedAssignament { block_level: usize, var_index: usize, value: FullValue },
    OptimizedAssignament { var_index: usize, value: FullValue },
    FnCall(ASTFunction),
    ReturnCall(FullValue),
}

#[derive(Clone)]
struct ExecutingContext {
    pub(crate) variables: Vec<RuntimeVariable>,
}

impl ExecutingContext {
    fn execute_block(&mut self, block: &Statement) -> Result<Option<MoonValue>, RuntimeError> {
        log::trace!("Executing block:\n{block:#?}");
        log::trace!("Variables at this point are:\n{:#?}", self.variables);
        match block {
            Statement::WhileBlock { condition, statements } => {
                while self.resolve_value(condition.clone())?.try_into()
                    .map_err(|_| RuntimeError::CannotTurnPredicateToBool { type_of_statement: "while", function_error_message: "".to_string() })? {
                    for statement in statements.iter() {
                        if let Some(res) = self.execute_block(statement)? {
                            return Ok(Some(res));
                        }
                    }
                }
            }
            Statement::IfElseBlock { conditional_statements: conditional_blocks } => {
                for block in conditional_blocks {
                    let boolean : bool = self.resolve_value(block.condition.clone())?.try_into()
                        .map_err(|_| RuntimeError::CannotTurnPredicateToBool { type_of_statement: "if", function_error_message: "".to_string() })?;
                    if boolean {
                        log::trace!("Executing statements of if block:\n{:#?}", block.statements);
                        for statement in block.statements.iter() {
                            if let Some(res) = self.execute_block(statement)? {
                                return Ok(Some(res));
                            }
                        }
                        return Ok(None);
                    }
                }
            }
            Statement::UnoptimizedAssignament { .. } => { unreachable!() }
            Statement::OptimizedAssignament { var_index, value } => {
                self.variables[*var_index] = RuntimeVariable::new(self.resolve_value(value.clone())?)
            }
            Statement::FnCall(function) => {
                function.function.execute_iter(function.args.iter().map(|arg| self.resolve_value(arg.clone())))?;
            }
            Statement::ReturnCall(value) => {
                return Ok(Some(self.resolve_value(value.clone())?));
            }
        }
        Ok(None)
    }

    fn resolve_value(&mut self, value: FullValue) -> Result<MoonValue, RuntimeError> {
        Ok(match value {
            FullValue::Null => MoonValue::Null,
            FullValue::Boolean(bool) => MoonValue::Boolean(bool),
            FullValue::Decimal(decimal) => MoonValue::Decimal(decimal),
            FullValue::Integer(integer) => MoonValue::Integer(integer),
            FullValue::String(string) => MoonValue::String(string),
            FullValue::Array(value) => {
                let mut res = Vec::with_capacity(value.len());
                for value in value.into_iter().map(|value| self.resolve_value(value)) {
                    match value {
                        Ok(value) => res.push(value),
                        Err(error) => return Err(error),
                    }
                }
                MoonValue::Array(res)
            }
            FullValue::Function(function) =>
                function.function.execute_iter(function.args.iter()
                    .map(|arg| self.resolve_value(arg.clone())))?,
            FullValue::Variable { .. } => unreachable!(),
            FullValue::DirectVariable(variable_index) => {
                let variable = mem::replace(&mut self.variables[variable_index].value, FullValue::Null);
                let res = self.resolve_value(variable)?;
                self.variables[variable_index] = RuntimeVariable::new(FullValue::from(res.clone()));
                res
            }
        })
    }
}

#[derive(Clone)]
/// Allows to execute an AST contents and to also push input variables.
pub struct ASTExecutor<'ast> {
    ast: &'ast AST,
    context: ExecutingContext,
}

impl<'ast> ASTExecutor<'ast> {

    pub(crate) fn new(ast: &'ast AST) -> Self {
        Self { ast, context: ExecutingContext { variables: ast.variables.clone() } }
    }

    /// Pushes a variable to this executor, if it is possible, it's preferred for you to push
    /// variables either as constants on the [crate::Engine], or in the [crate::ContextBuilder].
    pub fn push_variable<Name: ToString, Variable: Into<MoonValue>>(mut self, name: Name, variable: Variable) -> Self {
        let (name, variable) = (name.to_string(), variable.into());
        if let Some(variable_index) = self.ast.parameterized_variables.get(&name) {
            self.context.variables[*variable_index] = RuntimeVariable::from(FullValue::from(variable));
        }
        self
    }

    /// Executes the AST.
    pub fn execute(mut self) -> Result<MoonValue, RuntimeError> {
        for block in self.ast.statements.iter() {
            if let Some(res) = self.context.execute_block(&block)? {
                return Ok(res);
            }
        }
        Ok(MoonValue::Null)
    }
}
