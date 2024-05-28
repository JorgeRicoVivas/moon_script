use std::str::FromStr;

use pest::iterators::Pair;

use crate::{ASTFunction, Base, ContextBuilder, ExecutingContext, Rule};
use crate::external_utils::on_error_iter::IterOnError;

#[derive(Clone, PartialEq, Debug)]
pub enum ReducedValue {
    Null,
    Boolean(bool),
    Integer(i128),
    Decimal(f64),
    String(String),
    Array(Vec<ReducedValue>),
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
                .current_known_value.type_name(context_builder),
        }.to_string()
    }

    pub(crate) fn is_function(&self) -> bool {
        match self {
            Self::Function { .. } => true,
            _ => false
        }
    }

    fn resolve_value<'selflf>(&'selflf self, execution_context: &'selflf ExecutingContext) -> Result<ReducedValue, ()> {
        Ok(match self {
            FullValue::Null => ReducedValue::Null,
            FullValue::Boolean(bool) => ReducedValue::Boolean(*bool),
            FullValue::Decimal(decimal) => ReducedValue::Decimal(*decimal),
            FullValue::Integer(integer) => ReducedValue::Integer(*integer),
            FullValue::String(string) => ReducedValue::String(string.clone()),
            FullValue::Array(value) => {
                let mut res = Vec::with_capacity(value.len());
                for value in value.iter().map(|value| value.resolve_value(execution_context)) {
                    match value {
                        Ok(value) => res.push(value),
                        Err(error) => return Err(error),
                    }
                }
                ReducedValue::Array(res)
            }
            FullValue::Function(function) => {
                let mut reduced_args = Vec::with_capacity(function.args.len());
                for value in function.args.iter().map(|value| value.resolve_value(execution_context)) {
                    match value {
                        Ok(value) => reduced_args.push(value),
                        Err(error) => return Err(error),
                    }
                }
                (function.function)(reduced_args).unwrap()
            }
            FullValue::Variable { block_level, var_index } => execution_context.variables[*block_level][*var_index].value.clone(),
        })
    }

    fn is_simple_value(&self) -> bool {
        match self {
            FullValue::Null | FullValue::Boolean(_) | FullValue::Decimal(_) |
            FullValue::Integer(_) | FullValue::String(_) => true,
            FullValue::Array(values) => values.iter().all(|value| value.is_simple_value()),
            _ => false
        }
    }

    fn resolve_value_no_context(self) -> ReducedValue {
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
    let res = match token.as_rule() {
        Rule::BINARY_OPERATION => {
            let res = &base.binary_operation_parser
                .map_primary(|primary| {
                    build_value_token(primary, base, context)
                })
                .map_infix(|lhs, op, rhs| {
                    let operator = op.as_str();
                    println!("Found op {operator} left {lhs:?}, right {rhs:?}");
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

                    Ok(if function.can_inline && lhs.is_simple_value() && rhs.is_simple_value() {
                        let (lhs, rhs) = (lhs.resolve_value_no_context(), rhs.resolve_value_no_context());
                        FullValue::from((function.function)(vec![lhs, rhs])
                            .map_err(|err| vec![format!("Inlining error: {err}")])?
                        )
                    } else {
                        FullValue::Function(ASTFunction { function: function.function, args: vec![lhs, rhs] })
                    })
                })
                .parse(token.clone().into_inner());
            res.clone()
        }

        Rule::UNARY_OPERATION => {
            let mut token = token.clone().into_inner();
            let operator = token.next().unwrap().as_str();
            let value = token.next().unwrap();
            let value = build_value_token(value, base, context)?;
            let function = base.find_unary_operator(operator)
                .ok_or_else(|| vec![format!("Could not find binary operator {operator}")])?;
            Ok(if function.can_inline && value.is_simple_value() {
                let reduced_value = value.resolve_value_no_context();
                FullValue::from((function.function)(vec![reduced_value]).map_err(|err| vec![err])?)
            } else {
                FullValue::Function(ASTFunction { function: function.function, args: vec![value] })
            })
        }

        Rule::ARRAY => {
            let mut errors = Vec::new();
            let res = token.clone().into_inner().map(|pair| build_value_token(pair, base, context))
                .on_errors(|error| errors.extend(error.into_iter()))
                .collect();
            if !errors.is_empty() {
                return Err(errors);
            }
            Ok(FullValue::Array(res))
        }
        Rule::fncall => {
            let mut errors = Vec::new();
            let mut token = token.clone().into_inner();
            let mut object_type = None;
            let mut module = None;
            let mut function_name = token.as_str();
            loop {
                let current_token = token.next().unwrap();
                let current_token_as_str = current_token.as_str();

                match current_token.as_rule() {
                    Rule::fncall_object => object_type = Some(context.find_variable(&current_token_as_str)
                        .map(|(_, _, var)| var)
                        .ok_or_else(|| vec![format!("Variable {current_token_as_str} not in scope")])?
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
            Ok(if function.can_inline && args.iter().all(|arg| arg.is_simple_value()) {
                let inlined_res = (function.function)(args.into_iter().map(|arg| arg.resolve_value_no_context()).collect())
                    .map_err(|error_description| vec![format!("Could not inline function due to {error_description}")]);
                FullValue::from(inlined_res?)
            } else {
                FullValue::Function(ASTFunction { function: function.function, args })
            })
        }
        Rule::ident => {
            let (block_level, var_index, variable) =
                context.find_variable(token.as_str())
                    .ok_or_else(|| vec![format!("Variable {} not in scope", token.as_str())])?;
            Ok(if variable.current_known_value.is_simple_value() {
                variable.current_known_value.clone()
            } else {
                FullValue::Variable { block_level, var_index }
            })
        }
        Rule::null => Ok(FullValue::Null),
        Rule::boolean => Ok(FullValue::Boolean(token.as_str().eq("true") || token.as_str().eq("yes"))),
        Rule::decimal => Ok(FullValue::Decimal(f64::from_str(token.as_str())
            .map_err(|p| vec![
                format!("Couldn't parse {} into a decimal number between {} and {}", token.as_str(), f64::MIN, f64::MAX)
            ])?))
        ,
        Rule::integer => Ok(FullValue::Integer(i128::from_str(token.as_str())
            .map_err(|p| vec![
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
    println!("Parsed token {:?} = {} into value {res:?}", token.as_rule(), token.as_str());
    res
}
