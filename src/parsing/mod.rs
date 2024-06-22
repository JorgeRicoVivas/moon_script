use alloc::fmt::Debug;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::mem;

use pest::iterators::Pair;
use pest_derive::Parser;
use simple_detailed_error::SimpleError;

use statement_parsing::WalkInput;

use crate::engine::context::ContextBuilder;
use crate::engine::Engine;
use crate::execution::ast::{AST, Statement};
use crate::execution::RuntimeVariable;
use crate::function::{ToAbstractFunction, VBFunction};
use crate::HashMap;
use crate::value::FullValue;

pub(crate) mod value_parsing;
pub(crate) mod statement_parsing;
pub mod error;

#[derive(Parser)]
#[grammar = "language_definition.pest"]
pub struct SimpleParser;

pub struct FunctionInfo {
    can_inline_result: bool,
    function: VBFunction,
    return_type_name: Option<String>,
}

impl FunctionInfo {
    pub(crate) fn new<Dummy, Params, ReturnValue, Function, AbstractFunction: ToAbstractFunction<Params, ReturnValue, Function, Dummy>>
    (function: AbstractFunction) -> Self {
        Self::new_raw(function.abstract_function())
    }

    pub(crate) fn new_raw(function: VBFunction) -> Self {
        Self { function, return_type_name: None, can_inline_result: false }
    }

    pub(crate) fn inline(mut self) -> FunctionInfo {
        self.can_inline_result = true;
        self
    }
}


pub struct FunctionDefinition {
    pub(crate) associated_type_name: Option<String>,
    pub(crate) module_name: Option<String>,
    pub(crate) function_name: String,
    pub(crate) function_info: FunctionInfo,
}

impl FunctionDefinition {
    pub fn new<Name: Into<String>, Dummy, Params, ReturnValue, Function, AbstractFunction: ToAbstractFunction<Params, ReturnValue, Function, Dummy>>
    (function_name: Name, function: AbstractFunction) -> FunctionDefinition {
        Self {
            function_info: FunctionInfo::new_raw(function.abstract_function()),
            function_name: function_name.into(),
            module_name: None,
            associated_type_name: None,
        }
    }
    pub fn module_name<Name: Into<String>>(mut self, module_name: Name) -> FunctionDefinition {
        self.module_name = Some(module_name.into());
        self
    }
    pub fn associated_type_name<Name: Into<String>>(mut self, associated_type_name: Name) -> FunctionDefinition {
        self.associated_type_name = Some(associated_type_name.into());
        self
    }

    pub fn inline(mut self) -> FunctionDefinition {
        self.function_info.can_inline_result = true;
        self
    }

    pub fn knwon_return_type_name<Name: ToString>(mut self, return_type_name: Name) -> FunctionDefinition {
        self.function_info.return_type_name = Some(return_type_name.to_string());
        self
    }
}


fn optimize_variables(context: &mut ContextBuilder, inlineable_variables: Vec<(String, usize)>, statements: &mut Vec<Statement>) -> (Vec<RuntimeVariable>, HashMap<String, usize>) {
    let mut variables = context.take_all_variables().into_iter()
        .flat_map(|(block_level, variables)| {
            variables.into_iter().enumerate()
                .map(move |(var_index, variable)| ((block_level, var_index), variable))
        }).collect::<HashMap<_, _>>();

    let mut used_variables = HashMap::new();

    statements.iter_mut().for_each(|statement| {
        statement_parsing::walk_statement(&mut |input| {
            match input {
                WalkInput::Statement(block) => {
                    match block {
                        Statement::UnoptimizedAssignament { block_level, var_index, .. } => {
                            if !used_variables.contains_key(&(*block_level, *var_index)) {
                                log::trace!("Found used variable of block {block_level} and index {var_index}");
                                let variable = variables.remove(&(*block_level, *var_index)).unwrap();
                                log::trace!(" - Variable: {variable:?})");
                                used_variables.insert((*block_level, *var_index), variable);
                            }
                        }
                        _ => {}
                    }
                }
                WalkInput::Value(value) => {
                    match value {
                        FullValue::Variable { block_level, var_index } => {
                            if !used_variables.contains_key(&(*block_level, *var_index)) {
                                log::trace!("Found used variable of block {block_level} and index {var_index}");
                                let variable = variables.remove(&(*block_level, *var_index)).unwrap();
                                log::trace!(" - Variable: {variable:?})");
                                used_variables.insert((*block_level, *var_index), variable);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }, statement)
    });

    let mut used_variables = used_variables.into_iter().collect::<Vec<_>>();
    used_variables.sort_by(|((block_a, index_a), _), ((block_b, index_b), _)| {
        block_a.cmp(block_b).then_with(|| index_a.cmp(index_b))
    });

    let used_variables_and_new_indexes = used_variables.into_iter()
        .enumerate()
        .map(|(index, ((block_level, var_level), variable))| ((block_level, var_level), (index, variable)))
        .collect::<HashMap<_, _>>();

    let parameterized_variables = inlineable_variables.into_iter()
        .filter(|(_, index)| used_variables_and_new_indexes.contains_key(&(0, *index)))
        .map(|(name, index)| {
            let final_variable_index = used_variables_and_new_indexes.get(&(0, index)).unwrap().0;
            (name, final_variable_index)
        })
        .collect();

    statements.iter_mut().for_each(|statement| {
        statement_parsing::walk_statement(&mut |input| {
            match input {
                WalkInput::Statement(block) => {
                    match block {
                        Statement::UnoptimizedAssignament { block_level, var_index, value } => {
                            let direct_index = used_variables_and_new_indexes.get(&(*block_level, *var_index)).unwrap().0;
                            log::trace!("Substitued variable of assignament for block {block_level} and index {var_index} for simplified index {direct_index}");
                            *block = Statement::OptimizedAssignament { var_index: direct_index, value: mem::replace(value, FullValue::Null) };
                        }
                        _ => {}
                    }
                }
                WalkInput::Value(value) => {
                    match value {
                        FullValue::Variable { block_level, var_index } => {
                            let direct_index = used_variables_and_new_indexes.get(&(*block_level, *var_index)).unwrap().0;
                            log::trace!("Substitued variable of value for block {block_level} and index {var_index} for simplified index {direct_index}");
                            *value = FullValue::DirectVariable(direct_index);
                        }
                        _ => {}
                    }
                }
            }
        }, statement)
    });

    let mut used_variables_and_new_indexes = used_variables_and_new_indexes.into_iter()
        .map(|(_, variable)| variable)
        .collect::<Vec<_>>();
    used_variables_and_new_indexes.sort_by_key(|(index, _)| *index);

    let variables = used_variables_and_new_indexes.into_iter()
        .map(|(_, variable)| RuntimeVariable { value: variable.first_value })
        .collect::<Vec<_>>();
    (variables, parameterized_variables)
}

pub fn build_ast<'input>(token: Pair<'input, Rule>, base: &Engine, mut context: ContextBuilder) -> Result<AST, Vec<SimpleError<'input>>> {
    if token.as_rule() != Rule::BASE_STATEMENTS {}
    let statements_tokens = token.into_inner().next().unwrap();
    context.started_parsing = true;


    let inlineable_variables = context.in_use_variables.get(0).map(|(_, variables)| {
        variables.iter().enumerate()
            .filter(|(_, variable)| { variable.current_known_value.is_none() })
            .map(|(block_0_var_index, variable)| (variable.name.clone(), block_0_var_index))
            .collect::<Vec<_>>()
    }).unwrap_or_default();
    let mut statements = statement_parsing::build_token(statements_tokens, base, &mut context)?;

    let (variables, parameterized_variables) = optimize_variables(&mut context, inlineable_variables, &mut statements);
    Ok(AST { statements, variables, parameterized_variables })
}

fn line_and_column_of_token(token: &Pair<Rule>, context: &mut ContextBuilder) -> (usize, usize) {
    let mut line_and_column = token.line_col();
    if context.parsing_position_column_is_fixed || line_and_column.0 <= 1 {
        line_and_column = (line_and_column.0 + context.start_parsing_position_offset.0, line_and_column.1 + context.start_parsing_position_offset.1)
    } else {
        line_and_column = (line_and_column.0 + context.start_parsing_position_offset.0, line_and_column.1)
    }
    line_and_column
}


pub(crate) trait AddSourceOfError<'input> {
    fn add_where_error(self, input: &'input str, line_and_column: (usize, usize)) -> Self;
}


impl<'input, V> AddSourceOfError<'input> for Result<V, Vec<SimpleError<'input>>> {
    fn add_where_error(self, input: &'input str, line_and_column: (usize, usize)) -> Self {
        self.map_err(|errors| errors.into_iter().map(|error| {
            if error.current_at().is_none() { error.at(input).start_point_of_error(line_and_column.0, line_and_column.1) } else { error }
        }).collect::<Vec<_>>())
    }
}
