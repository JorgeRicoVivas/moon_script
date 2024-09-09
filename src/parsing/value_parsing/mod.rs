use alloc::collections::VecDeque;
use alloc::string::ToString;
use alloc::vec::Vec;
use alloc::{format, vec};
use core::mem;
use core::str::FromStr;

use pest::iterators::Pair;
use simple_detailed_error::{SimpleError, SimpleErrorDetail};

use crate::engine::context::ContextBuilder;
use crate::engine::Engine;
use crate::execution::ASTFunction;
use crate::external_utils::on_error_iter::IterOnError;
use crate::function::ToAbstractFunction;
use crate::parsing::error::ASTBuildingError;
use crate::parsing::{FunctionInfo, Rule};
use crate::value::{FullValue, MoonValue};

pub fn build_value_token<'input>(mut token: Pair<'input, Rule>, base: &Engine, context: &mut ContextBuilder) -> Result<FullValue, Vec<SimpleError<'input>>> {
    while token.as_rule().eq(&Rule::VALUE) {
        token = token.into_inner().next().unwrap()
    }
    let token_str = token.as_str();
    let token_rule = token.as_rule();
    log::trace!("Parsing complex token {token_rule:?} = {token_str}");
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
        Rule::ARRAY_ACCESS => {
            let mut token = token.into_inner();
            let mut value = build_value_token(token.next().unwrap(), base, context)?;
            for index_token in token.into_iter() {
                let index = usize::from_str(index_token.as_str())
                    .map_err(|_| vec![ASTBuildingError::CannotParseInteger { value: index_token.as_str(), lower_bound: usize::MIN as i128, upper_bound: usize::MAX as i128 }
                        .into()])?;
                let array_access_function = FunctionInfo {
                    can_inline_result: true,
                    function: (|moon_value: MoonValue, index: usize| -> Result<MoonValue, String> {
                        match moon_value {
                            MoonValue::Array(array) => array
                                .get(index)
                                .ok_or(format!("Index {index} it's out of bounds for array of length {}", array.len()))
                                .cloned(),
                            value => Err(format!("Tried accessing an index of an Array, while value is not an array, (Value: {value:?})"))
                        }
                    }).abstract_function(),
                    return_type_name: None,
                };
                value = decompress_function("array_access", vec![value, FullValue::from(MoonValue::from(index))], &array_access_function)?;
            }
            Ok(value)
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
            let mut object: Option<FullValue> = None;
            let mut object_type : Option<String> = None;
            let mut module = None;
            let function_name: &str;
            loop {
                let current_token = token.next().unwrap();
                let current_token_as_str = current_token.as_str();
                match current_token.as_rule() {
                    Rule::fncall_object => {
                        let (t_object_type, t_object) = context.find_variable(&current_token_as_str)
                            .map(|(block_level, var_index, value)|{
                                let type_name = value.associated_type_name.clone();
                                let value = FullValue::Variable { block_level, var_index };
                                (type_name, value)
                            })
                            .or_else(|| base.constants().get(current_token_as_str)
                                .map(|constant| (constant.type_name.clone(), FullValue::from(constant.value.clone())))
                            )
                            .ok_or_else(|| vec![ASTBuildingError::VariableNotInScope { variable_name: current_token_as_str }.into()])?;
                        object = Some(t_object);
                        object_type = Some(t_object_type
                            .ok_or_else(|| vec![ASTBuildingError::CouldntInlineVariableOfUnknownType { variable_name: current_token_as_str }.into()])?
                        );
                    }
                    Rule::fncall_module_name => module = Some(current_token_as_str),
                    Rule::fncall_function_name => {
                        function_name = current_token_as_str;
                        break;
                    }
                    _ => { panic!() }
                }
            }
            let mut args = token
                .map(|argument| build_value_token(argument, base, context))
                .on_errors(|error| errors.extend(error.into_iter()))
                .collect::<Vec<_>>();
            if let Some(variable) = object {
                args.insert(0, variable);
            }
            let function = base.find_function(object_type.clone(), module, function_name)
                .ok_or_else(|| vec![ASTBuildingError::FunctionNotFound { function_name, associated_to_type: object_type.clone(), module }.into()])?;
            Ok(decompress_function(function_name, args, function)?)
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
                Ok(FullValue::from(value.value.clone()))
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

fn decompress_function<'fn_name, 'fn_info>(function_name: &'fn_name str, args: Vec<FullValue>, function: &'fn_info FunctionInfo) -> Result<FullValue, Vec<SimpleError<'fn_name>>> {
    Ok(if function.can_inline_result && args.iter().all(|arg| arg.is_simple_value()) {
        let inlined_res = function.function.execute_iter(args.into_iter().map(|arg| Ok(arg.resolve_value_no_context())))
            .map_err(|runtime_error| vec![ASTBuildingError::CouldntInlineFunction { function_name, runtime_error }.into()])?;
        FullValue::from(inlined_res)
    } else {
        FullValue::Function(ASTFunction { function: function.function.clone(), args })
    })
}

//noinspection RsBorrowChecker
pub(crate) fn parse_property<'input>(idents: Pair<'input, Rule>, base: &Engine, context: &mut ContextBuilder, prepend_on_last_property: Option<&'static str>, mut extra_value_for_last_property: Option<FullValue>) -> Result<FullValue, Vec<SimpleError<'input>>> {
    let mut idents = idents.into_inner();
    let variable = idents.next().unwrap();

    let (mut type_name, mut value) = context.find_variable(variable.as_str())
        .map(|(block_level, var_index, variable)| {
            let type_of_var = variable.associated_type_name.clone();
            let value = if variable.inlineable_value().as_ref().is_some_and(|known_value| known_value.is_simple_value()) {
                variable.inlineable_value().unwrap()
            } else {
                FullValue::Variable { block_level, var_index }
            };
            (type_of_var, value)
        })
        .or_else(|| base.constants().get(variable.as_str())
            .map(|constant|
                (constant.type_name.clone(), FullValue::from(constant.value.clone()))))
        .ok_or_else(|| vec![ASTBuildingError::VariableNotInScope { variable_name: variable.as_str() }.into()])?;


    let mut idents_and_params = idents.collect::<VecDeque<_>>();
    while !idents_and_params.is_empty() {
        let property = idents_and_params.pop_front().unwrap();
        let is_last_ident = idents_and_params.iter().all(|rule| rule.as_rule() != Rule::ident);
        let prepend = if !is_last_ident || prepend_on_last_property.is_none() { "get_" } else { prepend_on_last_property.unwrap() };
        let prepended = format!("{prepend}{}", property.as_str());

        let function = base.find_function(type_name.clone(), None, &*prepended)
            .or_else(|| base.find_function(type_name.clone(), None, property.as_str()))
            .ok_or_else(|| vec![ASTBuildingError::PropertyFunctionNotFound {
                preferred_property_to_find: prepended,
                original_property: property.as_str(),
                typename: type_name.clone(),
            }.into()])?;
        let mut args = vec![value];
        if idents_and_params.front().as_ref().is_some_and(|rule| rule.as_rule() == Rule::property_params) {
            for arg in idents_and_params.pop_front().unwrap().into_inner().map(|value| build_value_token(value, base, context)) {
                args.push(arg?);
            }
        }
        if is_last_ident && extra_value_for_last_property.is_some() {
            args.push(mem::take(&mut extra_value_for_last_property).unwrap());
        }
        type_name = function.return_type_name.clone();
        value = if function.can_inline_result && args.iter().all(|arg| arg.is_simple_value()) {
            function.function.execute_iter(args.into_iter().map(|arg| Ok(arg.resolve_value_no_context())))
                .map_err(|err| vec![err.into()])?.into()
        } else {
            FullValue::Function(ASTFunction { function: function.function.clone(), args })
        }
    }
    Ok(value)
}
