use std::mem;
use std::ops::Range;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::block_parsing::AST;
use crate::block_parsing::value_parsing::{FullValue, ReducedValue};
use crate::execution::Block;
use crate::FUNCTION_ELEMENTS_LEN;

const OPTIMIZED_AST_CONTENT_TYPE_BLOCK: u8 = 0;
const OPTIMIZED_AST_CONTENT_TYPE_VALUE: u8 = 1;
const OPTIMIZED_AST_CONTENT_TYPE_FUNCTION: u8 = 2;

#[derive(Clone, Debug)]
struct Direction<const CONTENT_TYPE: u8> {
    dir: usize,
}

impl<const CONTENT_TYPE: u8> From<MultiDirection<CONTENT_TYPE>> for Direction<CONTENT_TYPE> {
    fn from(value: MultiDirection<CONTENT_TYPE>) -> Self {
        Direction { dir: value.start }
    }
}

#[derive(Clone, Debug)]
struct MultiDirection<const CONTENT_TYPE: u8> {
    start: usize,
    len: usize,
}

impl<const CONTENT_TYPE: u8> MultiDirection<CONTENT_TYPE> {
    fn iter(&self) -> Range<usize> {
        (self.start..(self.start + self.len)).into_iter()
    }
}

#[derive(Clone, Debug)]
pub enum OptimizedBlock {
    WhileBlock {
        condition: Direction<OPTIMIZED_AST_CONTENT_TYPE_VALUE>,
        statements: MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_BLOCK>,
    },
    IfElseBlock {
        condition: Direction<OPTIMIZED_AST_CONTENT_TYPE_VALUE>,
        positive_case_statements: MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_BLOCK>,
        negative_case_statements: MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_BLOCK>,
    },
    OptimizedAssignament {
        var_index: usize,
        value: Direction<OPTIMIZED_AST_CONTENT_TYPE_VALUE>,
    },
    FnCall(OptimizedASTFunction),
    ReturnCall(Direction<OPTIMIZED_AST_CONTENT_TYPE_VALUE>),
}

#[derive(Clone, Debug)]
struct OptimizedASTFunction {
    function: fn(Vec<ReducedValue>) -> Result<ReducedValue, String>,
    args: MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_VALUE>,
}

#[derive(Debug, Clone)]
enum OptimizedVariable {
    Value(ReducedValue),
    OtherVariable(usize),
    ASTValue(Direction<OPTIMIZED_AST_CONTENT_TYPE_VALUE>),
}

#[derive(Debug, Clone)]
pub enum OptimizedFullValue {
    Null,
    Boolean(bool),
    Integer(i128),
    Decimal(f64),
    String(String),
    Array(MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_VALUE>),
    Function(OptimizedASTFunction),
    DirectVariable(usize),
}

#[derive(Debug, Clone)]
struct OptimizedRuntimeVariable {
    value: OptimizedVariable,
}

#[derive(Debug, Clone)]
pub struct OptimizedAST {
    variables: Vec<OptimizedRuntimeVariable>,
    parameterized_variables: FxHashMap<String, usize>,

    statements: MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_BLOCK>,
    blocks: Vec<OptimizedBlock>,
    values: Vec<OptimizedFullValue>,
}

impl From<AST> for OptimizedAST {
    fn from(mut unoptimized_ast: AST) -> Self {
        let original_statements = mem::take(&mut unoptimized_ast.statements);
        let mut res = Self {
            variables: Vec::new(),
            parameterized_variables: unoptimized_ast.parameterized_variables,
            statements: MultiDirection { len: 0, start: 0 },
            blocks: vec![],
            values: vec![],
        };
        res.statements = res.optimize_blocks(original_statements);
        res.variables = unoptimized_ast.variables.into_iter().map(|value| {
            OptimizedRuntimeVariable { value: OptimizedVariable::ASTValue(res.optimize_values(vec![value.value]).into()) }
        }).collect();
        res
    }
}

impl OptimizedAST {
    fn optimize_blocks(&mut self, blocks: Vec<Block>) -> MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_BLOCK> {
        let blocks = blocks.into_iter().map(|block| {
            match block {
                Block::WhileBlock { condition, statements } =>
                    OptimizedBlock::WhileBlock {
                        condition: self.optimize_values(vec![condition]).into(),
                        statements: self.optimize_blocks(statements),
                    },
                Block::IfElseBlock { condition, positive_case_statements, negative_case_statements } =>
                    OptimizedBlock::IfElseBlock {
                        condition: self.optimize_values(vec![condition]).into(),
                        positive_case_statements: self.optimize_blocks(positive_case_statements),
                        negative_case_statements: self.optimize_blocks(negative_case_statements),
                    },
                Block::OptimizedAssignament { var_index, value } =>
                    OptimizedBlock::OptimizedAssignament { var_index, value: self.optimize_values(vec![value]).into() },
                Block::FnCall(function) => {
                    OptimizedBlock::FnCall(OptimizedASTFunction {
                        function: function.function,
                        args: self.optimize_values(function.args),
                    })
                }
                Block::ReturnCall(value) =>
                    OptimizedBlock::ReturnCall(self.optimize_values(vec![value]).into()),
                Block::UnoptimizedAssignament { .. } => { unreachable!() }
            }
        }).collect::<Vec<_>>();
        let values_len = blocks.len();
        let start = self.blocks.len();
        self.blocks.extend(blocks.into_iter());
        MultiDirection { start, len: values_len }
    }

    fn optimize_values(&mut self, values: Vec<FullValue>) -> MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_VALUE> {
        let values = values.into_iter().map(|value| {
            match value {
                FullValue::Null => OptimizedFullValue::Null,
                FullValue::Boolean(v) => OptimizedFullValue::Boolean(v),
                FullValue::Integer(v) => OptimizedFullValue::Integer(v),
                FullValue::Decimal(v) => OptimizedFullValue::Decimal(v),
                FullValue::String(v) => OptimizedFullValue::String(v),
                FullValue::Array(v) => OptimizedFullValue::Array(self.optimize_values(v)),
                FullValue::Function(v) =>
                    OptimizedFullValue::Function(OptimizedASTFunction {
                        function: v.function,
                        args: self.optimize_values(v.args),
                    }),
                FullValue::DirectVariable(v) => OptimizedFullValue::DirectVariable(v),
                FullValue::Variable { .. } => unreachable!()
            }
        }).collect::<Vec<_>>();
        let values_len = values.len();
        let start = self.values.len();
        self.values.extend(values.into_iter());
        MultiDirection { start, len: values_len }
    }

    pub fn executor(&self) -> OptimizedASTExecutor<'_> {
        OptimizedASTExecutor::new(self)
    }
}


pub struct OptimizedExecutingContext {
    pub(crate) variables: Vec<OptimizedRuntimeVariable>,
}

pub struct OptimizedASTExecutor<'ast> {
    ast: &'ast OptimizedAST,
    context: OptimizedExecutingContext,
}

impl<'ast> OptimizedASTExecutor<'ast> {
    pub(crate) fn new(ast: &'ast OptimizedAST) -> Self {
        Self { ast, context: OptimizedExecutingContext { variables: ast.variables.clone() } }
    }

    pub fn push_variable<Name: ToString, Variable: Into<ReducedValue>>(mut self, name: Name, variable: Variable) -> Self {
        let (name, variable) = (name.to_string(), variable.into());
        if let Some(variable_index) = self.ast.parameterized_variables.get(&name) {
            let context = &mut self.context;
            context.variables[*variable_index] = OptimizedRuntimeVariable { value: OptimizedVariable::Value(variable.into()) };
        }
        self
    }

    pub fn execute(mut self) -> Result<ReducedValue, String> {
        for block in self.ast.statements.iter() {
            if let Some(res) = self.context.execute_block(&self.ast.blocks[block], &self.ast)? {
                return Ok(res);
            }
        }
        Ok(ReducedValue::Null)
    }
}

impl OptimizedExecutingContext {
    fn execute_block(&mut self, block: &OptimizedBlock, ast: &OptimizedAST) -> Result<(Option<ReducedValue>), String> {
        match block {
            OptimizedBlock::WhileBlock { condition, statements } => {
                while self.resolve_value(condition.dir, ast)?.try_into().map_err(|_| "Couldn't solve a while loop's condition".to_string())? {
                    for statement in statements.iter().map(|block_index| &ast.blocks[block_index]) {
                        if let Some(res) = self.execute_block(statement, ast)? {
                            return Ok(Some(res));
                        }
                    }
                }
            }
            OptimizedBlock::IfElseBlock { condition, positive_case_statements, negative_case_statements } => {
                if self.resolve_value(condition.dir, ast)?.try_into().map_err(|_| "Couldn't solve an if block's condition")? {
                    for statement in positive_case_statements.iter().map(|block_index| &ast.blocks[block_index]) {
                        if let Some(res) = self.execute_block(statement, ast)? {
                            return Ok(Some(res));
                        }
                    }
                } else {
                    for statement in negative_case_statements.iter().map(|block_index| &ast.blocks[block_index]) {
                        if let Some(res) = self.execute_block(statement, ast)? {
                            return Ok(Some(res));
                        }
                    }
                }
            }
            OptimizedBlock::OptimizedAssignament { var_index, value } =>
                self.variables[*var_index] = OptimizedRuntimeVariable { value: OptimizedVariable::Value(self.resolve_value(value.dir, ast)?) },
            OptimizedBlock::FnCall(function) => {
                let mut args = Vec::with_capacity(function.args.len);
                for value in function.args.iter().map(|value_dir| self.resolve_value(value_dir, ast)) {
                    args.push(value?);
                }
                (function.function)(args).map_err(|error| error)?;
            }
            OptimizedBlock::ReturnCall(value) => {
                let value = self.resolve_value(value.dir, ast)?;
                return Ok(Some(value));
            },
        }
        Ok(None)
    }

    fn resolve_value(&mut self, value_dir: usize, ast: &OptimizedAST) -> Result<ReducedValue, String> {
        Ok(match &ast.values[value_dir] {
            OptimizedFullValue::Null => ReducedValue::Null,
            OptimizedFullValue::Boolean(v) => ReducedValue::Boolean(v.clone()),
            OptimizedFullValue::Integer(v) => ReducedValue::Integer(v.clone()),
            OptimizedFullValue::Decimal(v) => ReducedValue::Decimal(v.clone()),
            OptimizedFullValue::String(v) => ReducedValue::String(v.clone()),
            OptimizedFullValue::Array(v) => {
                let mut res = Vec::with_capacity(v.len);
                for value in v.iter().map(|value_dir| self.resolve_value(value_dir, ast)) {
                    res.push(value?)
                }
                ReducedValue::Array(res)
            }
            OptimizedFullValue::Function(function) => {
                /*
                let mut reduced_args = SmallVec::<[ReducedValue; FUNCTION_ELEMENTS_LEN]>::with_capacity(function.args.len);
                for value in function.args.iter().map(|value_dir| self.resolve_value(value_dir, ast)) {
                    reduced_args.push(value?)
                }
                (function.function)(reduced_args).unwrap()
                return (function.function)(Vec::new());
                */

                let mut reduced_args = Vec::with_capacity(function.args.len);
                for value in function.args.iter().map(|value_dir| self.resolve_value(value_dir, ast)) {
                    reduced_args.push(value?)
                }
                (function.function)(reduced_args).unwrap()
            }
            OptimizedFullValue::DirectVariable(variable_index) => {
                self.resolve_variable(ast, *variable_index)?
            }
        })
    }

    fn resolve_variable(&mut self, ast: &OptimizedAST, variable_index: usize) -> Result<ReducedValue, String> {
        let mut should_inline = true;
        let value = match &self.variables[variable_index].value {
            OptimizedVariable::Value(value) => {
                should_inline = false;
                value.clone()
            }
            OptimizedVariable::OtherVariable(other_var_index) => { self.resolve_variable(ast, *other_var_index)? }
            OptimizedVariable::ASTValue(value_dir) => { self.resolve_value(value_dir.dir, ast)? }
        };
        if should_inline {
            self.variables[variable_index].value = OptimizedVariable::Value(value.clone());
        }
        Ok(value)
    }
}