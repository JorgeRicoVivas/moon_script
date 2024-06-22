use core::mem;
use alloc::fmt::Debug;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::execution::{ASTFunction, ConditionalStatements, RuntimeVariable};
use crate::execution::optimized_ast::OptimizedAST;
use crate::HashMap;
use crate::parsing::value_parsing::{FullValue, VBValue};

#[derive(Debug, Clone)]
pub struct AST {
    pub(crate) statements: Vec<Statement>,
    pub(crate) variables: Vec<RuntimeVariable>,
    pub(crate) parameterized_variables: HashMap<String, usize>,
}

impl AST {
    pub fn executor(&self) -> ASTExecutor<'_> {
        ASTExecutor::new(self)
    }

    pub fn to_optimized_ast(self) -> OptimizedAST {
        OptimizedAST::from(self)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Statement {
    WhileBlock { condition: FullValue, statements: Vec<Statement> },
    IfElseBlock { conditional_statements: Vec<ConditionalStatements> },
    UnoptimizedAssignament { block_level: usize, var_index: usize, value: FullValue },
    OptimizedAssignament { var_index: usize, value: FullValue },
    FnCall(ASTFunction),
    ReturnCall(FullValue),
}


pub struct ExecutingContext {
    pub(crate) variables: Vec<RuntimeVariable>,
}

impl ExecutingContext {
    fn execute_block(&mut self, block: &Statement) -> Result<Option<VBValue>, Vec<String>> {
        match block {
            Statement::WhileBlock { condition, statements } => {
                while self.resolve_value(condition.clone()).map_err(|e| vec![e])?.try_into().map_err(|_| vec!["Couldn't solve a while loop's condition".to_string()])? {
                    for statement in statements.iter() {
                        if let Some(res) = self.execute_block(statement)? {
                            return Ok(Some(res));
                        }
                    }
                }
            }
            Statement::IfElseBlock { conditional_statements: conditional_blocks } => {
                for block in conditional_blocks {
                    if self.resolve_value(block.condition.clone()).map_err(|e| vec![e])?.try_into().map_err(|_| vec!["Couldn't solve an if block's condition".to_string()])? {
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
                self.variables[*var_index] = RuntimeVariable::new(self.resolve_value(value.clone()).map_err(|e| vec![e])?)
            }
            Statement::FnCall(function) => {
                function.function.execute_iter(function.args.iter().map(|arg| self.resolve_value(arg.clone()))).map_err(|e| vec![e])?;
            }
            Statement::ReturnCall(value) => {
                return Ok(Some(self.resolve_value(value.clone()).map_err(|e| vec![e])?));
            }
        }
        Ok(None)
    }

    fn resolve_value(&mut self, value: FullValue) -> Result<VBValue, String> {
        Ok(match value {
            FullValue::Null => VBValue::Null,
            FullValue::Boolean(bool) => VBValue::Boolean(bool),
            FullValue::Decimal(decimal) => VBValue::Decimal(decimal),
            FullValue::Integer(integer) => VBValue::Integer(integer),
            FullValue::String(string) => VBValue::String(string),
            FullValue::Array(value) => {
                let mut res = Vec::with_capacity(value.len());
                for value in value.into_iter().map(|value| self.resolve_value(value)) {
                    match value {
                        Ok(value) => res.push(value),
                        Err(error) => return Err(error),
                    }
                }
                VBValue::Array(res)
            }
            FullValue::Function(function) =>
                function.function.execute_iter(function.args.iter().map(|arg| self.resolve_value(arg.clone())))?,
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

pub struct ASTExecutor<'ast> {
    ast: &'ast AST,
    context: ExecutingContext,
}

impl<'ast> ASTExecutor<'ast> {
    pub(crate) fn new(ast: &'ast AST) -> Self {
        Self { ast, context: ExecutingContext { variables: ast.variables.clone() } }
    }

    pub fn push_variable<Name: ToString, Variable: Into<VBValue>>(mut self, name: Name, variable: Variable) -> Self {
        let (name, variable) = (name.to_string(), variable.into());
        if let Some(variable_index) = self.ast.parameterized_variables.get(&name) {
            self.context.variables[*variable_index] = RuntimeVariable::from(FullValue::from(variable));
        }
        self
    }

    pub fn execute(mut self) -> Result<VBValue, Vec<String>> {
        for block in self.ast.statements.iter() {
            if let Some(res) = self.context.execute_block(&block)? {
                return Ok(res);
            }
        }
        Ok(VBValue::Null)
    }
}
