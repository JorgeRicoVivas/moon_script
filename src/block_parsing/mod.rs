use std::collections::HashMap;
use std::convert::Infallible;
use std::mem;

use itertools::Itertools;
use pest::iterators::Pair;
use pest::Parser;
use pest::pratt_parser::{Assoc, Op, PrattParser};
use pest_derive::Parser;
use rustc_hash::FxHashMap;

use crate::block_parsing::value_parsing::{build_value_token, FullValue, ReducedValue};
use crate::execution::{ASTExecutor, Block, RuntimeVariable};
use crate::external_utils::on_error_iter::IterOnError;
use crate::function::{ToAbstractFunction, VBFunction};
use crate::reduced_value_impl;
use crate::reduced_value_impl::impl_operators;

pub(crate) mod value_parsing;

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

    pub(crate) fn knwon_return_type_name<Name: ToString>(mut self, return_type_name: Name) -> FunctionInfo {
        self.return_type_name = Some(return_type_name.to_string());
        self
    }
}

pub struct Base {
    //CustomType->RustModule->FunctionName->fn()
    associated_functions: HashMap<String, HashMap<String, HashMap<String, FunctionInfo>>>,
    //RustModule->FunctionName->fn()
    functions: HashMap<String, HashMap<String, FunctionInfo>>,

    //CustomType->FunctionName->fn()
    built_in_associated_functions: HashMap<String, HashMap<String, FunctionInfo>>,
    //FunctionName->fn()
    built_in_functions: HashMap<String, FunctionInfo>,

    //OperatorName->Fn()
    binary_operators: HashMap<String, FunctionInfo>,
    //OperatorName->Fn()
    unary_operators: HashMap<String, FunctionInfo>,
    binary_operation_parser: PrattParser<Rule>,

    constants: HashMap<String, ReducedValue>,

}

impl Default for Base {
    fn default() -> Self {
        let mut res = Self {
            associated_functions: Default::default(),
            functions: Default::default(),
            built_in_associated_functions: Default::default(),
            built_in_functions: Default::default(),
            binary_operators: impl_operators::get_binary_operators().into_iter()
                .map(|(name,function)|{
                    (name.to_string(), FunctionInfo::new(function).inline())
                })
                .collect(),
            unary_operators: impl_operators::get_unary_operators().into_iter()
                .map(|(name,function)|{
                    (name.to_string(), FunctionInfo::new(function).inline())
                })
                .collect(),
            binary_operation_parser: PrattParser::new()
                .op(Op::infix(Rule::sum, Assoc::Left) | Op::infix(Rule::sub, Assoc::Left))
                .op(Op::infix(Rule::mul, Assoc::Left) | Op::infix(Rule::div, Assoc::Left))
                .op(Op::infix(Rule::rem, Assoc::Left))
                .op(Op::infix(Rule::eq, Assoc::Left) | Op::infix(Rule::neq, Assoc::Left)
                    | Op::infix(Rule::gt, Assoc::Left) | Op::infix(Rule::gte, Assoc::Left)
                    | Op::infix(Rule::lt, Assoc::Left) | Op::infix(Rule::lte, Assoc::Left))
                .op(Op::infix(Rule::or, Assoc::Left) | Op::infix(Rule::xor, Assoc::Left)
                    | Op::infix(Rule::and, Assoc::Left) | Op::infix(Rule::rem, Assoc::Left)),
            constants: Default::default(),
        };
        res.add_function(FunctionDefinition::new("print", |value: String| {
            println!("{value}");
        }));
        res.add_function(FunctionDefinition::new("println", |value: String| {
            println!("{value}");
        }));
        res
    }
}


pub struct FunctionDefinition {
    associated_type_name: Option<String>,
    module_name: Option<String>,
    function_name: String,
    function_info: FunctionInfo,
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

#[derive(Debug)]
pub enum ParsingError {
    Parsing(pest::error::Error<Rule>),
    CouldntBuildAST(Vec<String>),
}

impl Base {
    pub fn add_constant<Name: ToString, Value: Into<ReducedValue>>(&mut self, name: Name, value: Value) -> Option<ReducedValue> {
        self.constants.insert(name.to_string(), value.into())
    }

    pub fn add_function<Function: Into<FunctionDefinition>>(&mut self, function_definition: Function) {
        let function_definition = function_definition.into();
        match (function_definition.associated_type_name, function_definition.module_name) {
            (None, None) => {
                self.built_in_functions.insert(function_definition.function_name, function_definition.function_info);
            }
            (Some(associated_type), None) => {
                self.built_in_associated_functions.entry(associated_type).or_default()
                    .insert(function_definition.function_name, function_definition.function_info);
            }
            (None, Some(module_name)) => {
                self.functions.entry(module_name).or_default()
                    .insert(function_definition.function_name, function_definition.function_info);
            }
            (Some(associated_type), Some(module_name)) => {
                self.associated_functions.entry(associated_type).or_default()
                    .entry(module_name).or_default()
                    .insert(function_definition.function_name, function_definition.function_info);
            }
        }
    }

    pub fn parse(&self, input: &str, context_builder: ContextBuilder) -> Result<AST, ParsingError> {
        let successful_parse = SimpleParser::parse(Rule::BASE_STATEMENTS, input)
            .map_err(|e| ParsingError::Parsing(e))?
            .next().unwrap();
        build_ast(successful_parse.clone(), self, context_builder)
            .map_err(|errors| ParsingError::CouldntBuildAST(errors))
    }

    fn find_unary_operator(&self, operator_name: &str) -> Option<&FunctionInfo> {
        self.unary_operators.get(operator_name)
    }

    fn find_binary_operator(&self, operator_name: &str) -> Option<&FunctionInfo> {
        self.binary_operators.get(operator_name)
    }

    fn find_function(&self, type_name: Option<String>, module_name: Option<&str>, function_name: &str) -> Option<&FunctionInfo> {
        if let Some(type_name) = type_name {
            if let Some(module_name) = module_name.clone() {
                self.associated_functions.get(&type_name)
                    .map(|assoc_map| assoc_map.get(module_name)
                        .map(|module_map| module_map.get(function_name)))
                    .flatten().flatten()
            } else {
                let resolve_from_built_in_associated = self.built_in_associated_functions.get(&type_name)
                    .map(|assoc_map| assoc_map.get(function_name)).flatten();
                if resolve_from_built_in_associated.is_some() { return resolve_from_built_in_associated; }
                let resolve_from_users_associated_functions = self.associated_functions.get(&type_name)
                    .map(|assoc_map| assoc_map.iter()
                        .map(|(_, module_map)| module_map.get(function_name))
                        .next()
                    ).flatten().flatten();
                resolve_from_users_associated_functions
            }
        } else {
            if let Some(module_name) = module_name.clone() {
                self.functions.get(module_name)
                    .map(|module_map| module_map.get(function_name))
                    .flatten()
            } else {
                let resolve_from_built_in_functions = self.built_in_functions.get(function_name);
                if resolve_from_built_in_functions.is_some() { return resolve_from_built_in_functions; }
                let resolve_from_user_functions = self.functions.iter()
                    .map(|(_, module_map)| module_map.get(function_name))
                    .next().flatten();
                resolve_from_user_functions
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextBuilder {
    in_use_variables: Vec<(usize, Vec<CompiletimeVariableInformation>)>,
    past_variables: Vec<(usize, Vec<CompiletimeVariableInformation>)>,
    next_block_level: usize,
    started_parsing: bool,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        let mut res = Self {
            in_use_variables: vec![],
            past_variables: vec![],
            next_block_level: 0,
            started_parsing: false,
        };
        res.push_block_level();
        res
    }
}

impl ContextBuilder {
    fn push_block_level(&mut self) {
        self.in_use_variables.push((self.next_block_level, Vec::new()));
        self.next_block_level += 1;
    }

    fn pop_block_level(&mut self) {
        self.past_variables.push(self.in_use_variables.remove(self.in_use_variables.len() - 1));
    }

    fn take_all_variables(&mut self) -> Vec<(usize, Vec<CompiletimeVariableInformation>)> {
        let mut variables = mem::take(&mut self.in_use_variables);
        variables.extend(mem::take(&mut self.past_variables));
        variables
    }

    fn find_variable(&self, variable_name: &str) -> Option<(usize, usize, &CompiletimeVariableInformation)> {
        self.in_use_variables.iter().rev()
            .map(|(block_level, var)|
                (*block_level, var.iter().enumerate().rev().filter(|(_, var)| var.name.eq(variable_name)).next())
            )
            .filter(|(_, var)| var.is_some())
            .next()
            .map(|(index, v)| v.map(|(var_index, var)| (index, var_index, var)))
            .flatten()
    }

    pub fn push_variable<Variable: Into<CompiletimeVariableInformation>>(&mut self, variable: Variable) -> (usize, usize) {
        let variable = variable.into();
        if !self.started_parsing {
            let already_existing_variable_index = self.in_use_variables[0].1.iter().position(|int_variable| int_variable.name.eq(&variable.name));
            return if let Some(already_existing_variable_index) = already_existing_variable_index {
                self.in_use_variables[0].1[already_existing_variable_index] = variable;
                (0, already_existing_variable_index)
            } else {
                self.in_use_variables[0].1.push(variable);
                (0, self.in_use_variables[0].1.len() - 1)
            };
        }
        let last_block = self.in_use_variables.len() - 1;
        self.in_use_variables[last_block].1.push(variable);
        (self.in_use_variables[last_block].0, self.in_use_variables[last_block].1.len() - 1)
    }

    fn get_variable_at(&self, block_level: usize, var_index: usize) -> Option<&CompiletimeVariableInformation> {
        self.in_use_variables.iter()
            .filter(|(int_block_level, _)| block_level.eq(int_block_level))
            .map(|(_, block_variables)| block_variables.get(var_index))
            .next().flatten()
    }
}


#[derive(Debug, Clone)]
pub struct CompiletimeVariableInformation {
    name: String,
    associated_type_name: String,
    current_known_value: Option<FullValue>,
}

impl CompiletimeVariableInformation {
    pub fn new<Name: ToString>(name: Name) -> Self {
        let mut name = name.to_string();
        let parsed = SimpleParser::parse(Rule::ident, &*name);
        if parsed.is_err() || parsed.unwrap().as_str().len() < name.len() {
            name = "Wrong variable name".to_string();
        }
        Self {
            name,
            associated_type_name: "Unknown type".to_string(),
            current_known_value: None,
        }
    }

    pub fn value<Value: Into<ReducedValue>>(mut self, value: Value) -> Self {
        let value = value.into();
        self.associated_type_name = value.type_name().to_string();
        self.current_known_value = Some(FullValue::from(value));
        self
    }

    pub fn associated_type<Name: ToString>(mut self, name: Name) -> Self {
        let name = name.to_string();
        let parsed = SimpleParser::parse(Rule::ident, &*name);
        if parsed.is_err() || parsed.unwrap().as_str().len() < name.len() {
            return self;
        }
        self.associated_type_name = name;
        self
    }
}

impl From<CompiletimeVariableInformation> for RuntimeVariable {
    fn from(value: CompiletimeVariableInformation) -> Self {
        RuntimeVariable::new(value.current_known_value.unwrap_or(FullValue::Null))
    }
}

enum WalkInput<'selflf> {
    Block(&'selflf mut Block),
    Value(&'selflf mut FullValue),
}


fn walk_block<Action: FnMut(WalkInput)>(action: &mut Action, block: &mut Block) {
    action(WalkInput::Block(block));
    match block {
        //Block::Statements(statement) => statement.iter_mut().for_each(|statement| walk_block(action, statement)),
        Block::WhileBlock { condition, statements } => {
            walk_value(action, condition);
            statements.iter_mut().for_each(|statement| walk_block(action, statement));
        }
        Block::IfElseBlock { condition, positive_case_statements, negative_case_statements } => {
            walk_value(action, condition);
            positive_case_statements.iter_mut().for_each(|statement| walk_block(action, statement));
            negative_case_statements.iter_mut().for_each(|statement| walk_block(action, statement));
        }
        Block::FnCall(function) => function.args.iter_mut().for_each(|value| walk_value(action, value)),
        Block::ReturnCall(value) => walk_value(action, value),
        Block::UnoptimizedAssignament { value, .. } => walk_value(action, value),
        Block::OptimizedAssignament { value, .. } => walk_value(action, value),
    }
}

fn walk_value<Action: FnMut(WalkInput)>(action: &mut Action, value: &mut FullValue) {
    action(WalkInput::Value(value));
    match value {
        FullValue::Array(values) => values.iter_mut().for_each(|value| walk_value(action, value)),
        FullValue::Function(function) => function.args.iter_mut().for_each(|value| walk_value(action, value)),
        FullValue::Variable { .. } => {}
        _ => {}
    }
}


fn optimize_variables(context: &mut ContextBuilder, inlineable_variables: Vec<(String, usize)>, statements: &mut Vec<Block>) -> (Vec<RuntimeVariable>, FxHashMap<String, usize>) {
    let mut variables = context.take_all_variables().into_iter()
        .flat_map(|(block_level, variables)| {
            variables.into_iter().enumerate()
                .map(move |(var_index, variable)| ((block_level, var_index), variable))
        }).collect::<HashMap<_, _>>();

    let mut used_variables = HashMap::new();

    statements.iter_mut().for_each(|statement| {
        walk_block(&mut |input| {
            match input {
                WalkInput::Block(block) => {
                    match block {
                        Block::UnoptimizedAssignament { block_level, var_index, .. } => {
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

    let used_variables_and_new_indexes = used_variables.into_iter()
        .sorted_by(|((block_a, index_a), _), ((block_b, index_b), _)| {
            block_a.cmp(block_b).then_with(|| index_a.cmp(index_b))
        })
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
        walk_block(&mut |input| {
            match input {
                WalkInput::Block(block) => {
                    match block {
                        Block::UnoptimizedAssignament { block_level, var_index, value } => {
                            let direct_index = used_variables_and_new_indexes.get(&(*block_level, *var_index)).unwrap().0;
                            log::trace!("Substitued variable of assignament for block {block_level} and index {var_index} for simplified index {direct_index}");
                            *block = Block::OptimizedAssignament { var_index: direct_index, value: mem::replace(value, FullValue::Null) };
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

    let variables = used_variables_and_new_indexes.into_iter()
        .map(|(_, a)| a)
        .sorted_by_key(|(index, _)| *index)
        .map(|(_, variable)| Into::<RuntimeVariable>::into(variable))
        .collect::<Vec<_>>();
    (variables, parameterized_variables)
}


#[derive(Debug, Clone)]
pub struct AST {
    pub(crate) statements: Vec<Block>,
    pub(crate) variables: Vec<RuntimeVariable>,
    pub(crate) parameterized_variables: FxHashMap<String, usize>,
}

impl AST {
    pub fn executor(&self) -> ASTExecutor<'_> {
        ASTExecutor::new(self)
    }
}

pub fn build_ast(token: Pair<Rule>, base: &Base, mut context: ContextBuilder) -> Result<AST, Vec<String>> {
    if token.as_rule() != Rule::BASE_STATEMENTS {}
    let statements_tokens = token.into_inner().next().unwrap();
    context.started_parsing = true;


    let inlineable_variables = context.in_use_variables.get(0).map(|(_, variables)| {
        variables.iter().enumerate()
            .filter(|(_, variable)| { variable.current_known_value.is_none() })
            .map(|(block_0_var_index, variable)| (variable.name.clone(), block_0_var_index))
            .collect::<Vec<_>>()
    }).unwrap_or_default();
    let mut statements = build_token(statements_tokens, base, &mut context)?;
    let (variables, parameterized_variables) = optimize_variables(&mut context, inlineable_variables, &mut statements);
    Ok(AST { statements, variables, parameterized_variables })
}

fn build_token(token: Pair<Rule>, base: &Base, context: &mut ContextBuilder) -> Result<Vec<Block>, Vec<String>> {
    log::trace!("Parsing rule {:?} with contents: {}", token.as_rule(), token.as_str());
    match token.as_rule() {
        Rule::STATEMENTS => {
            parse_statements(token, base, context)
        }
        Rule::WHILE_BLOCK => {
            let mut pairs = token.into_inner();
            let predicate = build_value_token(pairs.next().unwrap().into_inner().next().unwrap(), base, context)?;
            context.push_block_level();
            let statements = parse_statements(pairs.next().unwrap(), base, context)?;
            context.pop_block_level();
            Ok(vec![Block::WhileBlock { condition: predicate, statements }])
        }
        Rule::RETURN_CALL => {
            let value = build_value_token(token.into_inner().next().unwrap(), base, context)?;
            Ok(vec![Block::ReturnCall(value)])
        }
        Rule::IF_BLOCK => {
            let mut pairs = token.into_inner();
            let predicate = build_value_token(pairs.next().unwrap().into_inner().next().unwrap(), base, context)?;
            let positive_branch = pairs.next().unwrap();
            let negative_branch = pairs.next();
            let res = if predicate.is_simple_value() {
                let value = predicate.resolve_value_no_context();
                let selected_branch: bool = value.clone().try_into()
                    .map_err(|_| vec![format!("Expected bool, found {value}")])?;
                if selected_branch {
                    context.push_block_level();
                    let statements = parse_statements(positive_branch, base, context);
                    context.pop_block_level();
                    statements
                } else {
                    context.push_block_level();
                    let statements = negative_branch
                        .map(|negative_branch| parse_statements(negative_branch, base, context))
                        .unwrap_or(Ok(Vec::new()));
                    context.pop_block_level();
                    statements
                }?
            } else {
                context.push_block_level();
                let positive_branch = parse_statements(positive_branch, base, context)?;
                context.pop_block_level();
                context.push_block_level();
                let negative_branch = negative_branch
                    .map(|negative_branch| parse_statements(negative_branch, base, context))
                    .unwrap_or(Ok(Vec::new()))?;
                context.pop_block_level();
                vec![Block::IfElseBlock { condition: predicate, positive_case_statements: positive_branch, negative_case_statements: negative_branch }]
            };
            Ok(res)
        }
        Rule::ASSIGNMENT => {
            let mut pairs = token.into_inner();
            let ident = pairs.next().unwrap();
            let value = build_value_token(pairs.next().unwrap(), &base, context)?;
            match ident.as_rule() {
                Rule::ident => {
                    if value.is_simple_value() {
                        let compiletime_variable_information = CompiletimeVariableInformation {
                            associated_type_name: value.type_name(context),
                            name: ident.as_str().to_string(),
                            current_known_value: Some(value),
                        };
                        context.push_variable(compiletime_variable_information);
                        Ok(Vec::new())
                    } else {
                        let compiletime_variable_information = CompiletimeVariableInformation {
                            associated_type_name: value.type_name(context),
                            name: ident.as_str().to_string(),
                            current_known_value: None,
                        };
                        let (block_level, var_index) = context.push_variable(compiletime_variable_information);
                        Ok(vec![Block::UnoptimizedAssignament { block_level, var_index, value }])
                    }
                }
                Rule::property => {
                    let prop = value_parsing::parse_property(ident, base, context, Some("set_"), Some(value))?;
                    match prop {
                        FullValue::Function(function) => {
                            Ok(vec![Block::FnCall(function)])
                        }
                        _ => Ok(Vec::new()),
                    }
                }
                _ => { unreachable!() }
            }
        }
        Rule::fncall => {
            let function = build_value_token(token, base, context)?;
            Ok(match function {
                FullValue::Function(function) => {
                    vec![Block::FnCall(function)]
                }
                _ => {
                    Vec::new()
                    //ignored, execution of unrequired functions isn't taken
                }
            })
        }
        _ => { unreachable!() }
    }
}

fn parse_statements(token: Pair<Rule>, base: &Base, context: &mut ContextBuilder) -> Result<Vec<Block>, Vec<String>> {
    let mut errors = Vec::new();
    let statements = token.into_inner().map(|token| build_token(token, base, context))
        .on_errors(|error| errors.extend(error))
        .flat_map(|blocks| blocks)
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(statements)
    } else {
        Err(errors)
    }
}
