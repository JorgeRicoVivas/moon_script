use alloc::collections::VecDeque;

use core::ops::Range;
use core::mem;
use alloc::fmt::Debug;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use crate::execution::ast::AST;
use crate::execution::ast::Statement;
use crate::execution::RuntimeError;
use crate::function::VBFunction;
use crate::HashMap;
use crate::parsing::value_parsing::{FullValue, VBValue};

const OPTIMIZED_AST_CONTENT_TYPE_BLOCK: u8 = 0;
const OPTIMIZED_AST_CONTENT_TYPE_VALUE: u8 = 1;

#[derive(Clone, Debug)]
pub(crate) struct Direction<const CONTENT_TYPE: u8> {
    pub(crate) dir: usize,
}

impl<const CONTENT_TYPE: u8> From<MultiDirection<CONTENT_TYPE>> for Direction<CONTENT_TYPE> {
    fn from(value: MultiDirection<CONTENT_TYPE>) -> Self {
        Direction { dir: value.start }
    }
}

#[derive(Clone, Debug, Default)]
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
enum OptimizedBlock {
    WhileBlock {
        condition: Direction<OPTIMIZED_AST_CONTENT_TYPE_VALUE>,
        statements: MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_BLOCK>,
    },
    IfElseBlocks {
        blocks: MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_BLOCK>,
    },
    IfBlock {
        condition: Direction<OPTIMIZED_AST_CONTENT_TYPE_VALUE>,
        statements: MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_BLOCK>,
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
    function: VBFunction,
    args: MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_VALUE>,
}

#[derive(Debug, Clone)]
enum OptimizedVariable {
    Value(VBValue),
    ASTValue(Direction<OPTIMIZED_AST_CONTENT_TYPE_VALUE>),
}

#[derive(Debug, Clone)]
enum OptimizedFullValue {
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

#[derive(Debug, Clone, Default)]
pub struct OptimizedAST {
    variables: Vec<OptimizedRuntimeVariable>,
    parameterized_variables: HashMap<String, usize>,

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
            blocks: Default::default(),
            values: Default::default(),
        };
        res.statements = res.optimize_blocks(original_statements);
        res.variables = unoptimized_ast.variables.into_iter().map(|value| {
            OptimizedRuntimeVariable { value: OptimizedVariable::ASTValue(res.optimize_values(vec![value.value]).into()) }
        }).collect();
        res
    }
}

impl OptimizedAST {
    fn optimize_blocks(&mut self, blocks: Vec<Statement>) -> MultiDirection<OPTIMIZED_AST_CONTENT_TYPE_BLOCK> {
        let blocks = blocks.into_iter().map(|block| {
            match block {
                Statement::WhileBlock { condition, statements } =>
                    OptimizedBlock::WhileBlock {
                        condition: self.optimize_values(vec![condition]).into(),
                        statements: self.optimize_blocks(statements),
                    },
                Statement::IfElseBlock { conditional_statements: conditional_blocks } => {
                    let if_blocks = conditional_blocks.into_iter().map(|block| OptimizedBlock::IfBlock {
                        condition: self.optimize_values(vec![block.condition]).into(),
                        statements: self.optimize_blocks(block.statements),
                    }).collect::<Vec<_>>();
                    let values_len = if_blocks.len();
                    let start = self.blocks.len();
                    self.blocks.extend(if_blocks.into_iter());
                    OptimizedBlock::IfElseBlocks { blocks: MultiDirection { start, len: values_len } }
                }
                Statement::OptimizedAssignament { var_index, value } =>
                    OptimizedBlock::OptimizedAssignament { var_index, value: self.optimize_values(vec![value]).into() },
                Statement::FnCall(function) => {
                    OptimizedBlock::FnCall(OptimizedASTFunction {
                        function: function.function,
                        args: self.optimize_values(function.args),
                    })
                }
                Statement::ReturnCall(value) =>
                    OptimizedBlock::ReturnCall(self.optimize_values(vec![value]).into()),
                Statement::UnoptimizedAssignament { .. } => { unreachable!() }
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


struct OptimizedExecutingContext {
    variables: Vec<OptimizedRuntimeVariable>,
}

pub struct OptimizedASTExecutor<'ast> {
    ast: &'ast OptimizedAST,
    context: OptimizedExecutingContext,
}

impl<'ast> OptimizedASTExecutor<'ast> {
    pub(crate) fn new(ast: &'ast OptimizedAST) -> Self {
        Self { ast, context: OptimizedExecutingContext { variables: ast.variables.clone() } }
    }

    pub fn push_variable<Variable: Into<VBValue>>(mut self, name: &str, variable: Variable) -> Self {
        if let Some(variable_index) = self.ast.parameterized_variables.get(name) {
            self.context.variables[*variable_index] = OptimizedRuntimeVariable { value: OptimizedVariable::Value(variable.into().into()) };
        }
        self
    }

    pub fn execute(mut self) -> Result<VBValue, RuntimeError> {
        for block in self.ast.statements.iter() {
            if let Some(res) = self.context.execute_block(&self.ast.blocks[block], &self.ast)? {
                return Ok(res);
            }
        }
        Ok(VBValue::Null)
    }

    pub fn execute_stack(mut self) -> Result<VBValue, RuntimeError> {
        let mut stacked_execution_blocks = VecDeque::with_capacity(25);
        self.ast.statements.iter().rev().for_each(|dir| stacked_execution_blocks.push_front(dir));
        while let Some(block_dir) = stacked_execution_blocks.pop_front() {
            match &self.ast.blocks[block_dir] {
                OptimizedBlock::WhileBlock { condition, statements } => {
                    if self.context.resolve_value(condition.dir, &self.ast)?.try_into()
                        .map_err(|_| RuntimeError::CannotTurnPredicateToBool { type_of_statement: "while", function_error_message: "".to_string() })?{
                        stacked_execution_blocks.push_front(block_dir);
                        statements.iter().rev().for_each(|dir| stacked_execution_blocks.push_front(dir));
                    }
                }
                OptimizedBlock::IfElseBlocks { blocks } => {
                    for if_block_dir in blocks.iter() {
                        match &self.ast.blocks[if_block_dir] {
                            OptimizedBlock::IfBlock { condition, statements } => {
                                if self.context.resolve_value(condition.dir, &self.ast)?.try_into()
                                    .map_err(|_| RuntimeError::CannotTurnPredicateToBool { type_of_statement: "if", function_error_message: "".to_string() })? {
                                    statements.iter().rev().for_each(|dir| stacked_execution_blocks.push_front(dir));
                                    break;
                                }
                            }
                            _ => { unreachable!("IfElseBlocks should contain just IfBlocks, yet, something else was found") }
                        }
                    }
                }
                OptimizedBlock::IfBlock { .. } => { unreachable!("IfBlocks should not used directly, but IfElseBlocks instead") }
                OptimizedBlock::OptimizedAssignament { var_index, value } => {
                    self.context.variables[*var_index] = OptimizedRuntimeVariable { value: OptimizedVariable::Value(self.context.resolve_value(value.dir, &self.ast)?) }
                }
                OptimizedBlock::FnCall(function) => {
                    function.function.execute_iter(function.args.iter().map(|value_dir| self.context.resolve_value(value_dir, &self.ast)))?;
                }
                OptimizedBlock::ReturnCall(value) => {
                    let value = self.context.resolve_value(value.dir, &self.ast)?;
                    return Ok(value);
                }
            }
        }
        Ok(VBValue::Null)
    }
}

impl OptimizedExecutingContext {
    fn execute_block(&mut self, block: &OptimizedBlock, ast: &OptimizedAST) -> Result<Option<VBValue>, RuntimeError> {
        match block {
            OptimizedBlock::WhileBlock { condition, statements } => {
                while self.resolve_value(condition.dir, ast)?.try_into()
                    .map_err(|_| RuntimeError::CannotTurnPredicateToBool { type_of_statement: "if", function_error_message: "".to_string() })?{
                    for statement in statements.iter().map(|block_index| &ast.blocks[block_index]) {
                        if let Some(res) = self.execute_block(statement, ast)? {
                            return Ok(Some(res));
                        }
                    }
                }
            }
            OptimizedBlock::IfBlock { .. } => { unreachable!("IfBlocks should not used directly, but IfElseBlocks instead") }
            OptimizedBlock::IfElseBlocks { blocks } => {
                for if_block_dir in blocks.iter() {
                    match &ast.blocks[if_block_dir] {
                        OptimizedBlock::IfBlock { condition, statements } => {
                            if self.resolve_value(condition.dir, ast)?.try_into()
                                .map_err(|_| RuntimeError::CannotTurnPredicateToBool { type_of_statement: "if", function_error_message: "".to_string() })?{
                                for statement in statements.iter().map(|block_index| &ast.blocks[block_index]) {
                                    if let Some(res) = self.execute_block(statement, ast)? {
                                        return Ok(Some(res));
                                    }
                                }
                                return Ok(None);
                            }
                        }
                        _ => { unreachable!("IfElseBlocks should contain just IfBlocks, yet, something else was found") }
                    }
                }
            }
            OptimizedBlock::OptimizedAssignament { var_index, value } =>
                self.variables[*var_index] = OptimizedRuntimeVariable { value: OptimizedVariable::Value(self.resolve_value(value.dir, ast)?) },
            OptimizedBlock::FnCall(function) => {
                function.function.execute_iter(function.args.iter().map(|value_dir| self.resolve_value(value_dir, ast)))?;
            }
            OptimizedBlock::ReturnCall(value) => {
                let value = self.resolve_value(value.dir, ast)?;
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    fn resolve_value(&mut self, value_dir: usize, ast: &OptimizedAST) -> Result<VBValue, RuntimeError> {
        Ok(match &ast.values[value_dir] {
            OptimizedFullValue::Null => VBValue::Null,
            OptimizedFullValue::Boolean(v) => VBValue::Boolean(v.clone()),
            OptimizedFullValue::Integer(v) => VBValue::Integer(v.clone()),
            OptimizedFullValue::Decimal(v) => VBValue::Decimal(v.clone()),
            OptimizedFullValue::String(v) => VBValue::String(v.clone()),
            OptimizedFullValue::Array(v) => {
                let mut res = Vec::with_capacity(v.len);
                for value in v.iter().map(|value_dir| self.resolve_value(value_dir, ast)) {
                    res.push(value?)
                }
                VBValue::Array(res)
            }
            OptimizedFullValue::Function(function) => {
                function.function.execute_iter(function.args.iter()
                    .map(|value_dir| self.resolve_value(value_dir, ast)))?
            }
            OptimizedFullValue::DirectVariable(variable_index) => {
                self.resolve_variable(ast, *variable_index)?
            }
        })
    }

    fn resolve_variable(&mut self, ast: &OptimizedAST, variable_index: usize) -> Result<VBValue, RuntimeError> {
        let mut should_inline = true;
        let value = match &self.variables[variable_index].value {
            OptimizedVariable::Value(value) => {
                should_inline = false;
                value.clone()
            }
            OptimizedVariable::ASTValue(value_dir) => { self.resolve_value(value_dir.dir, ast)? }
        };
        if should_inline {
            self.variables[variable_index].value = OptimizedVariable::Value(value.clone());
        }
        Ok(value)
    }
}