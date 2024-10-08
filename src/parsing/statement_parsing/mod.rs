use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use pest::iterators::Pair;
use simple_detailed_error::SimpleError;

use crate::engine::context::{InputVariable, ContextBuilder};
use crate::engine::Engine;
use crate::execution::ast::Statement;
use crate::execution::ConditionalStatements;
use crate::external_utils::on_error_iter::IterOnError;
use crate::parsing;
use crate::parsing::{AddSourceOfError, Rule, value_parsing};
use crate::parsing::error::ASTBuildingError;
use crate::parsing::value_parsing::build_value_token;
use crate::value::{FullValue, MoonValue};

pub enum WalkInput<'selflf> {
    Statement(&'selflf mut Statement),
    Value(&'selflf mut FullValue),
}

pub fn walk_statement<Action: FnMut(WalkInput)>(action: &mut Action, statement: &mut Statement) {
    action(WalkInput::Statement(statement));
    match statement {
        Statement::WhileBlock { condition, statements } => {
            walk_value(action, condition);
            statements.iter_mut().for_each(|statement| walk_statement(action, statement));
        }
        Statement::IfElseBlock { conditional_statements } => {
            conditional_statements.iter_mut().for_each(|statement| {
                walk_value(action, &mut statement.condition);
                statement.statements.iter_mut().for_each(|statement| walk_statement(action, statement));
            });
        }
        Statement::FnCall(function) => function.args.iter_mut().for_each(|value| walk_value(action, value)),
        Statement::ReturnCall(value) => walk_value(action, value),
        Statement::UnoptimizedAssignament { value, .. } => walk_value(action, value),
        Statement::OptimizedAssignament { value, .. } => walk_value(action, value),
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

pub fn build_token<'input>(token: Pair<'input, Rule>, base: &Engine, context: &mut ContextBuilder, is_last_token: bool) -> Result<Vec<Statement>, Vec<SimpleError<'input>>> {
    let token_str = token.as_str();
    let token_as_string = if log::Level::Trace <= log::STATIC_MAX_LEVEL && log::Level::Trace <= log::max_level(){
        Some(token_str.to_string())
    }else{
        None
    };
    let line_and_column = parsing::line_and_column_of_token(&token, context);
    log::trace!("Parsing statement rule {:?} with contents: {}", token.as_rule(), token.as_str());
    let token_rule = token.as_rule();
    let res = match &token_rule {
        Rule::STATEMENTS => {
            parse_statements(token, base, context, true)
        }
        Rule::WHILE_BLOCK => {
            let mut pairs = token.into_inner();
            let predicate_pair = pairs.next().unwrap().into_inner().next().unwrap();
            let predicate_str = predicate_pair.as_str();
            let predicate = build_value_token(predicate_pair, base, context).add_where_error(predicate_str, line_and_column)?;
            context.push_block_level();
            let statements = parse_statements(pairs.next().unwrap(), base, context, false)?;
            context.pop_block_level();
            Ok(vec![Statement::WhileBlock { condition: predicate, statements }])
        }
        Rule::RETURN_CALL => {
            let value = build_value_token(token.into_inner().next().unwrap(), base, context).add_where_error(token_str, line_and_column)?;
            Ok(vec![Statement::ReturnCall(value)])
        }
        Rule::IF_BLOCK => {
            let mut pairs = token.into_inner();

            let mut parsed_statements = Vec::new();

            let mut first_predicate_str = None;
            let mut is_parsing_predicate = true;
            while let Some(current_token) = pairs.next() {
                if is_parsing_predicate {
                    let is_last_else_with_no_predicate = current_token.as_rule() == Rule::STATEMENTS;
                    if is_last_else_with_no_predicate {
                        context.push_block_level();
                        parsed_statements.push(ConditionalStatements {
                            condition: FullValue::from(MoonValue::Boolean(true)),
                            statements: parse_statements(current_token, base, context, false)?,
                        });
                        context.pop_block_level();
                        break;
                    }
                    let predicate_pair = current_token.into_inner().next().unwrap();
                    let predicate_str = predicate_pair.as_str();
                    if first_predicate_str.is_none() {
                        first_predicate_str = Some(predicate_str);
                    }
                    let predicate = build_value_token(predicate_pair, base, context).add_where_error(predicate_str, line_and_column)?;
                    parsed_statements.push(ConditionalStatements { condition: predicate, statements: Vec::new() })
                } else {
                    context.push_block_level();
                    let statements = parse_statements(current_token, base, context, false)?;
                    parsed_statements.last_mut().unwrap().statements.extend(statements);
                    context.pop_block_level();
                }
                is_parsing_predicate = !is_parsing_predicate;
            }
            parsed_statements.retain(|block| !block.condition.is_constant_boolean_false());
            if parsed_statements.is_empty() {
                return Ok(Vec::new());
            }
            if parsed_statements.len() == 1 {
                let single_conditional_block = parsed_statements.swap_remove(0);
                if single_conditional_block.condition.is_simple_value() {
                    let condition = single_conditional_block.condition.resolve_value_no_context();
                    let should_execute: bool = TryFrom::try_from(condition).map_err(|_|
                        vec![ASTBuildingError::ConditionDoestNotResolveToBoolean { predicate: first_predicate_str.unwrap() }.into()])
                        .add_where_error(token_str, line_and_column)?;
                    if should_execute {
                        return Ok(single_conditional_block.statements);
                    } else {
                        return Ok(vec![]);
                    }
                } else {
                    return Ok(vec![Statement::IfElseBlock { conditional_statements: vec![single_conditional_block] }]);
                }
            }
            let first_block = parsed_statements.get(0).unwrap();
            let first_if_block_is_always_true = first_block.condition.is_constant_boolean_true();
            if first_if_block_is_always_true {
                return Ok(parsed_statements.swap_remove(0).statements);
            }
            if let Some(first_always_executed_block) = parsed_statements.iter().position(|block| block.condition.is_constant_boolean_true()) {
                let target_len = first_always_executed_block + 1;
                while parsed_statements.len() > target_len {
                    parsed_statements.remove(parsed_statements.len() - 1);
                }
            }
            Ok(vec![Statement::IfElseBlock { conditional_statements: parsed_statements }])
        }
        Rule::ASSIGNMENT => {
            let token_start = token.as_span().start();
            let mut pairs = token.into_inner();
            let ident = pairs.next().unwrap();
            let has_let = ident.as_span().start() > token_start;
            let declare_variable_as_new = has_let;

            match ident.as_rule() {
                Rule::ident => {
                    let value = build_value_token(pairs.next().unwrap(), &base, context).add_where_error(token_str, line_and_column)?;
                    if value.is_simple_value() {
                        let compiletime_variable_information = InputVariable {
                            associated_type_name: value.type_name(context),
                            name: ident.as_str().to_string(),
                            current_known_value: Some(value.clone()),
                            first_value: value,
                            type_is_valid_up_to_depth: context.current_depth(),
                            value_is_valid_up_to_depth: context.current_depth(),
                            can_inline: true,
                        };
                        context.push_variable_internal(compiletime_variable_information, declare_variable_as_new);
                        Ok(Vec::new())
                    } else {
                        let compiletime_variable_information = InputVariable {
                            associated_type_name: value.type_name(context),
                            name: ident.as_str().to_string(),
                            current_known_value: None,
                            first_value: FullValue::Null,
                            type_is_valid_up_to_depth: context.current_depth(),
                            value_is_valid_up_to_depth: context.current_depth(),
                            can_inline: false,
                        };
                        let (block_level, var_index) = context.push_variable_internal(compiletime_variable_information, declare_variable_as_new);
                        Ok(vec![Statement::UnoptimizedAssignament { block_level, var_index, value }])
                    }
                }
                Rule::property => {
                    let value = build_value_token(pairs.next().unwrap(), &base, context).add_where_error(token_str, line_and_column)?;
                    let prop = value_parsing::parse_property(ident, base, context, Some("set_"), Some(value))
                        .add_where_error(token_str, line_and_column)?;
                    match prop {
                        FullValue::Function(function) => {
                            Ok(vec![Statement::FnCall(function)])
                        }
                        _ => Ok(Vec::new()),
                    }
                }
                _ => { unreachable!() }
            }
        }
        Rule::fncall => {
            let function = build_value_token(token, base, context).add_where_error(token_str, line_and_column)?;
            Ok(match function {
                FullValue::Function(function) => {
                    vec![Statement::FnCall(function)]
                }
                _ => {
                    Vec::new()
                    //ignored, execution of unrequired functions isn't taken
                }
            })
        }
        Rule::VALUE => {
            let value = build_value_token(token, base, context)?;
            if is_last_token {
                Ok(vec![Statement::ReturnCall(value)])
            } else if let Some(function) = match value {
                FullValue::Function(function) => Some(function),
                _ => None
            } {
                Ok(vec![Statement::FnCall(function)])
            } else {
                Ok(Vec::new())
            }
        }
        _ => { unreachable!("Shouldn't have found a rule of type: {:?}={}", &token_rule, token_str) }
    };
    log::trace!("Parsing statement rule {:?} with contents: {} into {:?}", &token_rule, token_as_string.unwrap(), res);
    res
}

fn parse_statements<'input>(token: Pair<'input, Rule>, base: &Engine, context: &mut ContextBuilder, last_statement_is_final_statement: bool) -> Result<Vec<Statement>, Vec<SimpleError<'input>>> {
    let mut errors = Vec::new();
    let statements_token = token.into_inner();
    let last_token_index = statements_token.len().checked_sub(1).unwrap_or(0);
    let statements = statements_token.enumerate().map(|(token_number, token)| {
        let token_str = token.as_str();
        let line_and_column = parsing::line_and_column_of_token(&token, context);
        build_token(token, base, context, last_statement_is_final_statement && last_token_index == token_number).add_where_error(token_str, line_and_column)
    })
        .on_errors(|error| errors.extend(error))
        .flat_map(|statements| statements)
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(statements)
    } else {
        Err(errors)
    }
}
