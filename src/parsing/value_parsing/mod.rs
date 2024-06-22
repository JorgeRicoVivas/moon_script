use alloc::{format, vec};
use alloc::collections::VecDeque;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::mem;
use core::str::FromStr;

use pest::iterators::Pair;
use simple_detailed_error::{SimpleError, SimpleErrorDetail};

use crate::engine::context::ContextBuilder;
use crate::engine::Engine;
use crate::execution::ASTFunction;
use crate::external_utils::on_error_iter::IterOnError;
use crate::parsing::error::ASTBuildingError;
use crate::parsing::Rule;
use crate::value::FullValue;

pub fn build_value_token<'input>(mut token: Pair<'input, Rule>, base: &Engine, context: &mut ContextBuilder) -> Result<FullValue, Vec<SimpleError<'input>>> {
    while token.as_rule().eq(&Rule::VALUE) {
        token = token.into_inner().next().unwrap()
    }
    let token_str = token.as_str();
    let token_rule = token.as_rule();
    let res = match token.as_rule() {
        Rule::BINARY_OPERATION => {
            let res = &base.binary_operation_parser()
                .map_primary(|primary| {
                    build_value_token(primary, base, context)
                })
                .map_infix(|lhs, op, rhs| {
                    let operator = op.as_str();
                    log::trace!("Found op {operator} left {lhs:?}, right {rhs:?}");
                    let function = base.find_binary_operator(operator);

                    if function.is_none() || lhs.is_err() || rhs.is_err() {
                        let mut error_union = lhs.err().unwrap_or_default();
                        error_union.extend(rhs.err().unwrap_or_default().into_iter());
                        if function.is_none() {
                            error_union.push(ASTBuildingError::OperatorNotFound { operator }.to_simple_error());
                        }
                        return Err(error_union);
                    }
                    let (lhs, rhs, function) = (lhs.unwrap(), rhs.unwrap(), function.unwrap());

                    Ok(if function.can_inline_result && lhs.is_simple_value() && rhs.is_simple_value() {
                        let (lhs, rhs) = (lhs.resolve_value_no_context(), rhs.resolve_value_no_context());
                        FullValue::from(
                            function.function.execute_into_iter([Ok(lhs), Ok(rhs)].into_iter())
                                .map_err(|runtime_error| vec![ASTBuildingError::CouldntInlineBinaryOperator { operator, runtime_error }.into()])?
                        )
                    } else {
                        FullValue::Function(ASTFunction { function: function.function.clone(), args: vec![lhs, rhs] })
                    })
                })
                .parse(token.into_inner());
            res.clone()
        }
        Rule::UNARY_OPERATION => {
            let mut token = token.into_inner();
            let operator = token.next().unwrap().as_str();
            let value = token.next().unwrap();
            let value = build_value_token(value, base, context)?;
            let function = base.find_unary_operator(operator)
                .ok_or_else(|| vec![ASTBuildingError::OperatorNotFound { operator }.at(token_str)])?;
            Ok(if function.can_inline_result && value.is_simple_value() {
                let reduced_value = value.resolve_value_no_context();
                FullValue::from(
                    function.function.execute_iter([Ok(reduced_value)].into_iter())
                        .map_err(|runtime_error| vec![ASTBuildingError::CouldntInlineUnaryOperator { operator, runtime_error }.into()])?)
            } else {
                FullValue::Function(ASTFunction { function: function.function.clone(), args: vec![value] })
            })
        }
        Rule::ARRAY => {
            let mut errors = Vec::new();
            let res = token.into_inner().map(|pair| build_value_token(pair, base, context))
                .on_errors(|error| errors.extend(error.into_iter()))
                .collect();
            if !errors.is_empty() {
                return Err(errors);
            }
            Ok(FullValue::Array(res))
        }
        Rule::fncall => {
            let mut errors = Vec::new();
            let mut token = token.into_inner();
            let mut object_type = None;
            let mut module = None;
            let function_name: &str;
            loop {
                let current_token = token.next().unwrap();
                let current_token_as_str = current_token.as_str();

                match current_token.as_rule() {
                    Rule::fncall_object => {
                        object_type = Some(context.find_variable(&current_token_as_str)
                            .map(|(_, _, var)| var)
                            .ok_or_else(|| vec![ASTBuildingError::VariableNotInScope { variable_name: current_token_as_str }.into()])?
                            .associated_type_name.clone()
                            .ok_or_else(|| vec![ASTBuildingError::CouldntInlineVariableOfUnknownType { variable_name: current_token_as_str }.into()])?
                        )
                    }
                    Rule::fncall_module_name => module = Some(current_token_as_str),
                    Rule::fncall_function_name => {
                        function_name = current_token_as_str;
                        break;
                    }
                    _ => { panic!() }
                }
            }
            let args = token
                .map(|argument| build_value_token(argument, base, context))
                .on_errors(|error| errors.extend(error.into_iter()))
                .collect::<Vec<_>>();

            let function = base.find_function(object_type.clone(), module, function_name)
                .ok_or_else(|| vec![ASTBuildingError::FunctionNotFound { function_name, associated_to_type: object_type.clone(), module }.into()])?;
            Ok(if function.can_inline_result && args.iter().all(|arg| arg.is_simple_value()) {
                let inlined_res = function.function.execute_iter(args.into_iter().map(|arg| Ok(arg.resolve_value_no_context())))
                    .map_err(|runtime_error| vec![ASTBuildingError::CouldntInlineFunction { function_name, runtime_error }.into()])?;
                FullValue::from(inlined_res)
            } else {
                FullValue::Function(ASTFunction { function: function.function.clone(), args })
            })
        }
        Rule::ident => {
            let ident = token.as_str();
            if let Some((block_level, var_index, variable)) = context.find_variable(ident) {
                Ok(if variable.inlineable_value().is_some_and(|known_value| known_value.is_simple_value()) {
                    variable.inlineable_value().unwrap()
                } else {
                    FullValue::Variable { block_level, var_index }
                })
            } else if let Some(value) = base.constants().get(ident) {
                Ok(FullValue::from(value.clone()))
            } else {
                Err(vec![ASTBuildingError::VariableNotInScope { variable_name: ident }.to_simple_error()])
            }
        }
        Rule::property => Ok(parse_property(token, base, context, None, None)?),
        Rule::null => Ok(FullValue::Null),
        Rule::boolean => Ok(FullValue::Boolean(token.as_str().eq("true") || token.as_str().eq("yes"))),
        Rule::decimal => Ok(FullValue::Decimal(f64::from_str(token.as_str())
            .map_err(|_| vec![ASTBuildingError::CannotParseDecimal { value: token_str, lower_bound: f64::MIN, upper_bound: f64::MAX }.into()])?)),
        Rule::integer => Ok(FullValue::Integer(i128::from_str(token.as_str())
            .map_err(|_| vec![ASTBuildingError::CannotParseInteger { value: token_str, lower_bound: i128::MIN, upper_bound: i128::MAX }.into()])?)),
        Rule::string => {
            let mut string = token.as_str().to_string();
            string.remove(string.len() - 1);
            string.remove(0);
            Ok(FullValue::String(string))
        }
        _ => Ok(FullValue::Null),
    };
    log::trace!("Parsed token {token_rule:?} = {token_str} into value {res:?}");
    res
}

//noinspection RsBorrowChecker
pub(crate) fn parse_property<'input>(token: Pair<'input, Rule>, base: &Engine, context: &mut ContextBuilder, prepend_on_last_property: Option<&'static str>, mut extra_value_for_last_property: Option<FullValue>) -> Result<FullValue, Vec<SimpleError<'input>>> {
    let mut idents = token.into_inner();
    let variable = idents.next().unwrap();
    let (block_level, var_index, variable) =
        context.find_variable(variable.as_str())
            .ok_or_else(|| vec![ASTBuildingError::VariableNotInScope { variable_name: variable.as_str() }.into()])?;
    let mut last_associated_type_name = variable.associated_type_name.clone();
    let mut stack = if variable.inlineable_value().as_ref().is_some_and(|known_value| known_value.is_simple_value()) {
        variable.inlineable_value().unwrap()
    } else {
        FullValue::Variable { block_level, var_index }
    };
    let mut idents_and_params = idents.collect::<VecDeque<_>>();
    while !idents_and_params.is_empty() {
        let property = idents_and_params.pop_front().unwrap();
        let is_last_ident = idents_and_params.iter().all(|rule| rule.as_rule() != Rule::ident);
        let prepend = if !is_last_ident || prepend_on_last_property.is_none() { "get_" } else { prepend_on_last_property.unwrap() };
        let prepended = format!("{prepend}{}", property.as_str());

        let function = base.find_function(last_associated_type_name.clone(), None, &*prepended)
            .or_else(|| base.find_function(last_associated_type_name.clone(), None, property.as_str()))
            .ok_or_else(|| vec![ASTBuildingError::PropertyFunctionNotFound {
                preferred_property_to_find: prepended,
                original_property: property.as_str(),
                typename: last_associated_type_name.clone().unwrap_or_else(|| "Unknown type".to_string()),
            }.into()])?;
        let mut args = vec![stack];
        if idents_and_params.front().as_ref().is_some_and(|rule| rule.as_rule() == Rule::property_params) {
            for arg in idents_and_params.pop_front().unwrap().into_inner().map(|value| build_value_token(value, base, context)) {
                args.push(arg?);
            }
        }
        if is_last_ident && extra_value_for_last_property.is_some() {
            args.push(mem::take(&mut extra_value_for_last_property).unwrap());
        }
        last_associated_type_name = function.return_type_name.clone();
        stack = if function.can_inline_result && args.iter().all(|arg| arg.is_simple_value()) {
            function.function.execute_iter(args.into_iter().map(|arg| Ok(arg.resolve_value_no_context())))
                .map_err(|err| vec![err.into()])?.into()
        } else {
            FullValue::Function(ASTFunction { function: function.function.clone(), args })
        }
    }
    Ok(stack)
}
