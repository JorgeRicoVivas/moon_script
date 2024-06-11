use std::mem;

use crate::block_parsing::AST;
use crate::block_parsing::value_parsing::{FullValue, ReducedValue};
use crate::external_utils::on_error_iter::IterOnError;
use crate::function::VBFunction;

#[derive(Clone, Debug)]
pub struct ASTFunction {
    pub(crate) function: VBFunction,
    pub(crate) args: Vec<FullValue>,
}


#[derive(Clone, Debug)]
pub enum Block {
    WhileBlock { condition: FullValue, statements: Vec<Block> },
    IfElseBlock { condition: FullValue, positive_case_statements: Vec<Block>, negative_case_statements: Vec<Block> },
    UnoptimizedAssignament { block_level: usize, var_index: usize, value: FullValue },
    OptimizedAssignament { var_index: usize, value: FullValue },
    FnCall(ASTFunction),
    ReturnCall(FullValue),
}


pub struct ExecutingContext {
    pub(crate) variables: Vec<RuntimeVariable>,
}

impl ExecutingContext {
    fn execute_block(&mut self, block: &Block) -> Result<Option<ReducedValue>, Vec<String>> {
        match block {
            Block::WhileBlock { condition, statements } => {
                while self.resolve_value(condition.clone()).map_err(|e|vec![e])?.try_into().map_err(|_| vec!["Couldn't solve a while loop's condition".to_string()])? {
                    for statement in statements.iter() {
                        if let Some(res) = self.execute_block(statement)? {
                            return Ok(Some(res));
                        }
                    }
                }
            }
            Block::IfElseBlock { condition, positive_case_statements, negative_case_statements } => {
                if self.resolve_value(condition.clone()).map_err(|e|vec![e])?.try_into().map_err(|_| vec!["Couldn't solve an if block's condition".to_string()])? {
                    for statement in positive_case_statements.iter() {
                        if let Some(res) = self.execute_block(statement)? {
                            return Ok(Some(res));
                        }
                    }
                } else {
                    for statement in negative_case_statements.iter() {
                        if let Some(res) = self.execute_block(statement)? {
                            return Ok(Some(res));
                        }
                    }
                }
            }
            Block::UnoptimizedAssignament { .. } => { unreachable!() }
            Block::OptimizedAssignament { var_index, value } => {
                self.variables[*var_index] = RuntimeVariable::new(self.resolve_value(value.clone()).map_err(|e|vec![e])?)
            }
            Block::FnCall(function) => {
                function.function.execute_iter(function.args.iter().map(|arg|self.resolve_value(arg.clone()))).map_err(|e|vec![e])?;
            }
            Block::ReturnCall(value) => {
                return Ok(Some(self.resolve_value(value.clone()).map_err(|e|vec![e])?));
            }
        }
        Ok(None)
    }

    fn resolve_value(&mut self, value: FullValue) -> Result<ReducedValue, String> {
        Ok(match value {
            FullValue::Null => ReducedValue::Null,
            FullValue::Boolean(bool) => ReducedValue::Boolean(bool),
            FullValue::Decimal(decimal) => ReducedValue::Decimal(decimal),
            FullValue::Integer(integer) => ReducedValue::Integer(integer),
            FullValue::String(string) => ReducedValue::String(string),
            FullValue::Array(value) => {
                let mut res = Vec::with_capacity(value.len());
                for value in value.into_iter().map(|value| self.resolve_value(value)) {
                    match value {
                        Ok(value) => res.push(value),
                        Err(error) => return Err(error),
                    }
                }
                ReducedValue::Array(res)
            }
            FullValue::Function(function) =>
                function.function.execute_iter(function.args.iter().map(|arg|self.resolve_value(arg.clone())))?,
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

    pub fn push_variable<Name: ToString, Variable: Into<ReducedValue>>(mut self, name: Name, variable: Variable) -> Self {
        let (name, variable) = (name.to_string(), variable.into());
        if let Some(variable_index) = self.ast.parameterized_variables.get(&name) {
            self.context.variables[*variable_index] = RuntimeVariable::from(FullValue::from(variable));
        }
        self
    }

    pub fn execute(mut self) -> Result<ReducedValue, Vec<String>> {
        for block in self.ast.statements.iter() {
            if let Some(res) = self.context.execute_block(&block)? {
                return Ok(res);
            }
        }
        Ok(ReducedValue::Null)
    }
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

pub(crate) mod optimized;