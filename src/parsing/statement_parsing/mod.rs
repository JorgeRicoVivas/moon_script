use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use pest::iterators::Pair;
use simple_detailed_error::SimpleError;

use crate::engine::context::{CompiletimeVariableInformation, ContextBuilder};
use crate::engine::Engine;
use crate::execution::ast::Statement;
use crate::execution::ConditionalStatements;
use crate::external_utils::on_error_iter::IterOnError;
use crate::parsing;
use crate::parsing::{AddSourceOfError, Rule, value_parsing};
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

pub fn build_token<'input>(token: Pair<'input, Rule>, base: &Engine, context: &mut ContextBuilder) -> Result<Vec<Statement>, Vec<SimpleError<'input>>> {
    let token_str = token.as_str();
    let line_and_column = parsing::line_and_column_of_token(&token, context);
    log::trace!("Parsing rule {:?} with contents: {}", token.as_rule(), token.as_str());
    match token.as_rule() {
        Rule::STATEMENTS => {
            parse_statements(token, base, context)
        }
        Rule::WHILE_BLOCK => {
            let mut pairs = token.into_inner();
            context.forbid_variables_from_inlining();
            let predicate_pair = pairs.next().unwrap().into_inner().next().unwrap();
            let predicate_str = predicate_pair.as_str();
            let predicate = build_value_token(predicate_pair, base, context).add_where_error(predicate_str, line_and_column)?;
            context.permit_variables_to_inline();
            context.push_block_level();
            let statements = parse_statements(pairs.next().unwrap(), base, context)?;
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

            let mut is_parsing_predicate = true;
            while let Some(current_token) = pairs.next() {
                if is_parsing_predicate {
                    let is_last_else_with_no_predicate = current_token.as_rule() == Rule::STATEMENTS;
                    if is_last_else_with_no_predicate {
                        context.push_block_level();
                        parsed_statements.push(ConditionalStatements {
                            condition: FullValue::from(MoonValue::Boolean(true)),
                            statements: parse_statements(current_token, base, context)?,
                        });
                        context.pop_block_level();
                        break;
                    }
                    let predicate_pair = current_token.into_inner().next().unwrap();
                    let predicate_str = predicate_pair.as_str();
                    let predicate = build_value_token(predicate_pair, base, context).add_where_error(predicate_str, line_and_column)?;
                    parsed_statements.push(ConditionalStatements { condition: predicate, statements: Vec::new() })
                } else {
                    context.push_block_level();
                    let statements = parse_statements(current_token, base, context)?;
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
                return Ok(parsed_statements.swap_remove(0).statements);
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

            let value = build_value_token(pairs.next().unwrap(), &base, context).add_where_error(token_str, line_and_column)?;
            match ident.as_rule() {
                Rule::ident => {
                    if value.is_simple_value() {
                        let compiletime_variable_information = CompiletimeVariableInformation {
                            associated_type_name: value.type_name(context),
                            name: ident.as_str().to_string(),
                            current_known_value: Some(value.clone()),
                            first_value: value,
                            type_is_valid_up_to_depth: context.current_depth(),
                            value_is_valid_up_to_depth: context.current_depth(),
                            can_inline: true,
                            global_can_inline: context.variables_should_inline,
                        };
                        context.push_variable_internal(compiletime_variable_information, declare_variable_as_new);
                        Ok(Vec::new())
                    } else {
                        let compiletime_variable_information = CompiletimeVariableInformation {
                            associated_type_name: value.type_name(context),
                            name: ident.as_str().to_string(),
                            current_known_value: None,
                            first_value: FullValue::Null,
                            type_is_valid_up_to_depth: context.current_depth(),
                            value_is_valid_up_to_depth: context.current_depth(),
                            can_inline: false,
                            global_can_inline: context.variables_should_inline,
                        };
                        let (block_level, var_index) = context.push_variable_internal(compiletime_variable_information, declare_variable_as_new);
                        Ok(vec![Statement::UnoptimizedAssignament { block_level, var_index, value }])
                    }
                }
                Rule::property => {
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
        _ => { unreachable!("Shouldn't have found a rule of type: {:?}={}", token.as_rule(), token.as_str()) }
    }
}

fn parse_statements<'input>(token: Pair<'input, Rule>, base: &Engine, context: &mut ContextBuilder) -> Result<Vec<Statement>, Vec<SimpleError<'input>>> {
    let mut errors = Vec::new();
    let statements = token.into_inner().map(|token| {
        let token_str = token.as_str();
        let line_and_column = parsing::line_and_column_of_token(&token, context);
        build_token(token, base, context).add_where_error(token_str, line_and_column)
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
