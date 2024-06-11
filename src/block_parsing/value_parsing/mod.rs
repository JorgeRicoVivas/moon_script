use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::mem;
use std::str::FromStr;

use pest::iterators::Pair;

use crate::block_parsing::{Base, ContextBuilder};
use crate::execution::ASTFunction;
use crate::external_utils::on_error_iter::IterOnError;
use crate::Rule;

#[derive(Clone, PartialEq, Debug)]
pub enum ReducedValue {
    Null,
    Boolean(bool),
    Integer(i128),
    Decimal(f64),
    String(String),
    Array(Vec<ReducedValue>),
}

impl ReducedValue {
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Boolean(_) => "bool",
            Self::Integer(_) => "int",
            Self::Decimal(_) => "decimal",
            Self::String(_) => "string",
            Self::Array(_) => "array",
        }
    }
}

impl TryFrom<FullValue> for ReducedValue {
    type Error = ();
    fn try_from(value: FullValue) -> Result<Self, Self::Error> {
        Ok(match value {
            FullValue::Null => { ReducedValue::Null }
            FullValue::Boolean(v) => { ReducedValue::Boolean(v) }
            FullValue::Integer(v) => { ReducedValue::Integer(v) }
            FullValue::Decimal(v) => { ReducedValue::Decimal(v) }
            FullValue::String(v) => { ReducedValue::String(v) }
            FullValue::Array(v) => {
                let mut values = Vec::with_capacity(v.len());
                for value in v {
                    values.push(ReducedValue::try_from(value)?)
                };
                ReducedValue::Array(values)
            }
            _ => { return Err(()); }
        })
    }
}

impl Display for ReducedValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReducedValue::Null => f.write_str("null"),
            ReducedValue::Boolean(bool) => f.write_str(&*bool.to_string()),
            ReducedValue::Integer(int) => f.write_str(&*int.to_string()),
            ReducedValue::Decimal(dec) => f.write_str(&*dec.to_string()),
            ReducedValue::String(string) => f.write_str(&format!("\"{string}\"")),
            ReducedValue::Array(array) => {
                let mut result = String::new();
                result.push('[');
                let mut is_first_value = true;
                array.iter().for_each(|value| {
                    if is_first_value {
                        result.push_str(&format!("{value}"));
                        is_first_value = false;
                    } else {
                        result.push_str(&format!(", {value}"));
                    }
                });
                result.push(']');
                f.write_str(&*result)
            }
        }
    }
}


#[derive(Debug, Clone)]
pub enum FullValue {
    Null,
    Boolean(bool),
    Integer(i128),
    Decimal(f64),
    String(String),
    Array(Vec<FullValue>),
    Function(ASTFunction),
    Variable { block_level: usize, var_index: usize },
    DirectVariable(usize),
}

impl FullValue {
    pub(crate) fn type_name(&self, context_builder: &ContextBuilder) -> String {
        match self {
            FullValue::Null => "null",
            FullValue::Boolean(_) => "bool",
            FullValue::Integer(_) => "int",
            FullValue::Decimal(_) => "decimal",
            FullValue::String(_) => "string",
            FullValue::Array(_) => "array",
            FullValue::Function(_) => "function",
            FullValue::Variable { block_level, var_index } => return context_builder
                .get_variable_at(*block_level, *var_index).unwrap()
                .current_known_value.as_ref()
                .map(|know_value| know_value.type_name(context_builder))
                .unwrap_or_else(|| "Unknown Value Type".to_string()),
            FullValue::DirectVariable(_) => { unreachable!() }
        }.to_string()
    }

    pub(crate) fn is_simple_value(&self) -> bool {
        match self {
            FullValue::Null | FullValue::Boolean(_) | FullValue::Decimal(_) |
            FullValue::Integer(_) | FullValue::String(_) => true,
            FullValue::Array(values) => values.iter().all(|value| value.is_simple_value()),
            _ => false
        }
    }

    pub(crate) fn resolve_value_no_context(self) -> ReducedValue {
        match self {
            FullValue::Null => ReducedValue::Null,
            FullValue::Boolean(bool) => ReducedValue::Boolean(bool),
            FullValue::Decimal(decimal) => ReducedValue::Decimal(decimal),
            FullValue::Integer(integer) => ReducedValue::Integer(integer),
            FullValue::String(string) => ReducedValue::String(string),
            FullValue::Array(value) => ReducedValue::Array(value.into_iter()
                .map(|value| value.resolve_value_no_context())
                .collect()),
            _ => panic!()
        }
    }
}


pub fn build_value_token(mut token: Pair<Rule>, base: &Base, context: &mut ContextBuilder) -> Result<FullValue, Vec<String>> {
    while token.as_rule().eq(&Rule::VALUE) {
        token = token.into_inner().next().unwrap()
    }
    let token_str = token.as_str();
    let token_rule = token.as_rule();
    let res = match token.as_rule() {
        Rule::BINARY_OPERATION => {
            let res = &base.binary_operation_parser
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
                            error_union.push(format!("Could not find binary operator {operator}"));
                        }
                        return Err(error_union);
                    }
                    let (lhs, rhs, function) = (lhs.unwrap(), rhs.unwrap(), function.unwrap());

                    Ok(if function.can_inline_result && lhs.is_simple_value() && rhs.is_simple_value() {
                        let (lhs, rhs) = (lhs.resolve_value_no_context(), rhs.resolve_value_no_context());

                        FullValue::from(
                            function.function.execute_into_iter([Ok(lhs), Ok(rhs)].into_iter()).map_err(|err| vec![format!("Inlining error: {err}")])?
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
                .ok_or_else(|| vec![format!("Could not find binary operator {operator}")])?;
            Ok(if function.can_inline_result && value.is_simple_value() {
                let reduced_value = value.resolve_value_no_context();
                FullValue::from(
                    function.function.execute_iter([Ok(reduced_value)].into_iter())
                        .map_err(|err| vec![err])?)
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
            let line = token.as_str();
            let mut object_type = None;
            let mut module = None;
            let function_name: &str;
            loop {
                let current_token = token.next().unwrap();
                let current_token_as_str = current_token.as_str();

                match current_token.as_rule() {
                    Rule::fncall_object => object_type = Some(context.find_variable(&current_token_as_str)
                        .map(|(_, _, var)| var)
                        .ok_or_else(|| vec![format!("Variable {current_token_as_str} not in scope (when inlining function for line '{line}')")])?
                        .associated_type_name.to_string()),
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
                .ok_or_else(|| {
                    let mut base = "Could not find".to_string();
                    if let Some(object_type) = object_type {
                        base.push_str(&format!(" associated function {function_name} for object {object_type}"));
                    } else {
                        base.push_str(&format!(" function named {function_name}"));
                    }
                    if let Some(module) = module {
                        base.push_str(&format!(" in module {module}"));
                    }
                    vec![base]
                })?;
            Ok(if function.can_inline_result && args.iter().all(|arg| arg.is_simple_value()) {
                let inlined_res = function.function.execute_iter(args.into_iter().map(|arg| Ok(arg.resolve_value_no_context())))
                    .map_err(|error_description| vec![format!("Could not inline function due to {error_description}")])?;
                FullValue::from(inlined_res)
            } else {
                FullValue::Function(ASTFunction { function: function.function.clone(), args })
            })
        }
        Rule::ident => {
            let ident = token.as_str();
            if let Some((block_level, var_index, variable)) = context.find_variable(ident) {
                Ok(if variable.current_known_value.as_ref().is_some_and(|known_value| known_value.is_simple_value()) {
                    variable.current_known_value.clone().unwrap()
                } else {
                    FullValue::Variable { block_level, var_index }
                })
            } else if let Some(value) = base.constants.get(ident) {
                Ok(FullValue::from(value.clone()))
            } else {
                Err(vec![format!("Variable {} not in scope (When searching for ident)", ident)])
            }
        }
        Rule::property => Ok(parse_property(token, base, context, None, None)?),
        Rule::null => Ok(FullValue::Null),
        Rule::boolean => Ok(FullValue::Boolean(token.as_str().eq("true") || token.as_str().eq("yes"))),
        Rule::decimal => Ok(FullValue::Decimal(f64::from_str(token.as_str())
            .map_err(|_| vec![
                format!("Couldn't parse {} into a decimal number between {} and {}", token.as_str(), f64::MIN, f64::MAX)
            ])?))
        ,
        Rule::integer => Ok(FullValue::Integer(i128::from_str(token.as_str())
            .map_err(|_| vec![
                format!("Couldn't parse {} into a integer number between {} and {}", token.as_str(), i128::MIN, i128::MAX)
            ])?)),
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

pub(crate) fn parse_property(token: Pair<Rule>, base: &Base, context: &mut ContextBuilder, prepend_on_last_property: Option<&'static str>, mut extra_value_for_last_property: Option<FullValue>) -> Result<FullValue, Vec<String>> {
    let line = token.as_str();
    let mut idents = token.into_inner();
    let variable = idents.next().unwrap();
    let (block_level, var_index, variable) =
        context.find_variable(variable.as_str())
            .ok_or_else(|| vec![format!("Variable {} not in scope (When parsing property in line '{line}')", variable.as_str())])?;
    let mut last_associated_type_name = Some(variable.associated_type_name.clone());
    let mut stack = if variable.current_known_value.as_ref().is_some_and(|known_value| known_value.is_simple_value()) {
        variable.current_known_value.clone().unwrap()
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
            .ok_or_else(|| vec![format!(
                "There is no function {prepended} nor {} for the property {} whose type is {}",
                property.as_str(),
                property.as_str(),
                last_associated_type_name.clone().unwrap_or_else(|| "Unknown type".to_string()))]
            )?;
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
                .map_err(|_| vec![format!("Could not properly inline getter function matching property {}", property.as_str())])?
                .into()
        } else {
            FullValue::Function(ASTFunction { function: function.function.clone(), args })
        }
    }
    Ok(stack)
}
