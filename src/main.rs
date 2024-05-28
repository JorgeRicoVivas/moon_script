extern crate pest_derive;

use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;

use itertools::Itertools;
use pest::{Parser, RuleType};
use pest::iterators::Pair;
use pest::pratt_parser::{Assoc, Op, PrattParser};
use pest_derive::Parser;

use value_parsing::{FullValue, ReducedValue};

use crate::external_utils::on_error_iter::IterOnError;
use crate::external_utils::on_none_iter::IterOnNone;
use crate::value_parsing::build_value_token;

pub mod external_utils;

#[derive(Parser)]
#[grammar = "language_definition.pest"]
struct SimpleParser;

struct FunctionInfo {
    can_inline: bool,
    function: fn(Vec<ReducedValue>) -> Result<ReducedValue, String>,
}

struct Base {
    //CustomType->RustModule->FunctionName->fn()
    associated_functions: HashMap<String, HashMap<String, HashMap<String, FunctionInfo>>>,
    //RustModule->FunctionName->fn()
    functions: HashMap<String, HashMap<String, FunctionInfo>>,
    //OperatorName->Fn()
    binary_operators: HashMap<String, FunctionInfo>,
    //OperatorName->Fn()
    unary_operators: HashMap<String, FunctionInfo>,
    binary_operation_parser: PrattParser<Rule>,
}


impl Base {
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
                self.associated_functions.get(&type_name)
                    .map(|assoc_map| assoc_map.iter()
                        .map(|(_, module_map)| module_map.get(function_name))
                        .next()
                    ).flatten().flatten()
            }
        } else {
            if let Some(module_name) = module_name.clone() {
                self.functions.get(module_name)
                    .map(|module_map| module_map.get(function_name))
                    .flatten()
            } else {
                self.functions.iter()
                    .map(|(_, module_map)| module_map.get(function_name))
                    .next().flatten()
            }
        }
    }
}

#[derive(Debug)]
struct ContextBuilder {
    variables: Vec<Vec<CompiletimeVariableInformation>>,
}

impl ContextBuilder {

    fn find_variable(&self, variable_name: &str) -> Option<(usize, usize, &CompiletimeVariableInformation)> {
        self.variables.iter().rev().enumerate()
            .map(|(block_level, var)|
                (block_level, var.iter().enumerate().filter(|(_, var)| var.name.eq(variable_name)).next())
            )
            .next()
            .map(|(index, v)| v.map(|(var_index, var)| (index, var_index, var)))
            .flatten()
    }

    fn push_variable(&mut self, variable: CompiletimeVariableInformation) {
        let last_block_level = self.variables.len()-1;
        let block_level_variables = &mut self.variables[last_block_level];
        if let Some(already_exiting_variable) = block_level_variables.iter_mut().find(|var|variable.name.eq(&var.name)){
            *already_exiting_variable=variable;
        }else{
            block_level_variables.push(variable)
        }
    }

    fn get_variable_at(&self, block_level: usize, var_index: usize) -> Option<&CompiletimeVariableInformation> {
        self.variables.get(block_level).map(|block_variables| block_variables.get(var_index)).flatten()
    }

    fn get_mut_variable_at(&mut self, block_level: usize, var_index: usize) -> Option<&mut CompiletimeVariableInformation> {
        self.variables.get_mut(block_level).map(|block_variables| block_variables.get_mut(var_index)).flatten()
    }
}


#[derive(Debug)]
struct CompiletimeVariableInformation {
    name: String,
    associated_type_name: String,
    current_known_value: FullValue,
}

struct ExecutingContext {
    variables: Vec<Vec<RuntimeVariable>>,
}

struct RuntimeVariable {
    value: ReducedValue,
}

fn main() {
    println!("Start");
    let mut associated_functions: HashMap<String, HashMap<String, HashMap<String, FunctionInfo>>> = Default::default();
    associated_functions.entry("MyCustomType".to_string()).or_default()
        .entry("Mod".to_string()).or_default()
        .insert("object_function".to_string(),
                FunctionInfo {
                    can_inline: false,
                    function: |_| { Ok(ReducedValue::Boolean(true)) },
                });
    let mut functions: HashMap<String, HashMap<String, FunctionInfo>> = Default::default();
    functions.entry("Mod".to_string()).or_default()
        .insert("function".to_string(),
                FunctionInfo {
                    can_inline: false,
                    function: |_| { Ok(ReducedValue::Boolean(false)) },
                });

    let base = Base {
        associated_functions,
        functions,
        binary_operators: reduced_value_impl::impl_operators::get_binary_operators().into_iter()
            .map(|(function_name, operation)| {
                (function_name.to_string(), FunctionInfo {
                    can_inline: true,
                    function: operation,
                })
            })
            .collect(),
        unary_operators: reduced_value_impl::impl_operators::get_unary_operators().into_iter()
            .map(|(function_name, operation)| {
                (function_name.to_string(), FunctionInfo {
                    can_inline: true,
                    function: operation,
                })
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
    };

    let successful_parse = SimpleParser::parse(Rule::STATEMENTS, INPUT)
        .unwrap()
        .next()
        .unwrap();

    if successful_parse.as_str().len() < INPUT.len() {
        println!("Wrong parse");
        return;
    }


    let mut context = ContextBuilder {
        variables:
        vec![
            vec![
                CompiletimeVariableInformation {
                    name: "first_var".to_string(),
                    associated_type_name: "i64".to_string(),
                    current_known_value: FullValue::Integer(5020),
                },
                CompiletimeVariableInformation {
                    associated_type_name: "MyCustomType".to_string(),
                    name: "ident".to_string(),
                    current_known_value: FullValue::Integer(2050),
                }]
        ],
    };

    /*
    let array_input = "[ null empty false true !true yes no 3 1.5 .75 \"Text\" ident function( 1 2 3 ) ident.object_function( 1 5 ) [1 2 3] [[1 2 3][4 5 6][7 8 9]] ---ident 1+(2*3)+4 ]";
    let array_input = "[2*((1+2)*(3+4))/5  3]";
    let array_input = "[5/2 + 8/4]";
    let successful_parse = SimpleParser::parse(Rule::ARRAY, array_input)
        .unwrap()
        .next()
        .unwrap();

    if successful_parse.as_str().len() < array_input.len() {
        println!("Wrong parse");
        return;
    }
    println!("{successful_parse:#?}");

    let values = successful_parse.into_inner().map(|pair| {
        value_parsing::build_value_token(pair, &base, &mut context)
    }).collect::<Vec<_>>();
    */


    println!("---Simplified---");
    print_pairs_iter([successful_parse.clone()].into_iter(), 0);

    let res = build_token(successful_parse, &base, &mut context);
    println!("Context result: {context:#?}");
    println!("Result: {res:#?}");
}

mod reduced_value_impl;

pub(crate) mod value_parsing;

#[derive(Clone, Debug)]
struct ASTFunction {
    function: fn(Vec<ReducedValue>) -> Result<ReducedValue, String>,
    args: Vec<FullValue>,
}

#[derive(Clone, Debug)]
enum Block {
    Statements(Vec<Block>),
    Assignment { variable_index: usize, value: FullValue },
    FnCall(ASTFunction),
    Dummy,
}

const INPUT: &'static str = "
            let a = 1;
            a = 3;
            b = a+2;
            Mod/function(a, b);

            ";
/*
            if positive_branch {
                positive_action();
            } else {
                negative_action();
            }
            if single_branch {
                conditional_action();
            }
            while looped_condition {
                looping_action();
            }
 */

fn build_token(token: Pair<Rule>, base: &Base, context: &mut ContextBuilder) -> Result<Option<Block>, Vec<String>> {
    println!("Parsing rule {:?} with contents: {}", token.as_rule(), token.as_str());
    match token.as_rule() {
        Rule::STATEMENTS => {
            let mut errors = Vec::new();
            let statements = token.into_inner().map(|token| build_token(token, base, context))
                .on_errors(|error| errors.extend(error))
                .ignore_nones()
                .collect::<Vec<_>>();
            if !errors.is_empty() {
                return Err(errors);
            }
            Ok(Some(Block::Statements(statements)))
        }
        Rule::WHILE_BLOCK => {
            Err(Vec::new())
        }
        Rule::IF_BLOCK => {
            Err(Vec::new())
        }
        Rule::ASSIGNMENT => {
            let mut pairs = token.into_inner();
            let ident = pairs.next().unwrap().as_str();
            let value = build_value_token(pairs.next().unwrap(), &base, context)?;
            let compiletime_variable_information = CompiletimeVariableInformation {
                associated_type_name: value.type_name(context),
                name: ident.to_string(),
                current_known_value: value,
            };
            context.push_variable(compiletime_variable_information);
            Err(Vec::new())
        }
        Rule::fncall => {
            let function = build_value_token(token, base, context)?;
            Ok(match function {
                FullValue::Function(function) => {
                    Some(Block::FnCall(function))
                }
                _ => {
                    None
                    //ignored, execution of unrequired functions isn't taken
                }
            })
        }

        Rule::PREDICATE => { Err(Vec::new()) }
        Rule::POSITIVE_ACTION => { Err(Vec::new()) }
        Rule::NEGATIVE_ACTION => { Err(Vec::new()) }

        _ => { panic!() }
    }
}

fn print_pairs_iter<'a, Rule: RuleType, RuleIter: Iterator<Item=Pair<'a, Rule>>>(pairs: RuleIter, ident: u8) {
    pairs.into_iter().for_each(|p| {
        let tabs = (0..ident).into_iter().map(|_| " - ").join("");
        println!("{tabs}{:?} = {}", p.as_rule(), p.as_str());
        print_pairs_iter(p.into_inner(), ident + 1);
    });
}