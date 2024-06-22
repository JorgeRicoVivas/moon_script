use alloc::string::{String, ToString};
use pest::Parser;
use pest::pratt_parser::{Assoc, Op, PrattParser};
use simple_detailed_error::SimpleErrorDetail;

use context::ContextBuilder;

use crate::execution::ast::AST;
use crate::{HashMap, parsing};
use crate::parsing::{FunctionDefinition, FunctionInfo, Rule, SimpleParser};
use crate::parsing::error::ParsingError;
use crate::parsing::value_parsing::VBValue;
use crate::reduced_value_impl::impl_operators;

pub mod context;

pub struct Engine {
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

    constants: HashMap<String, VBValue>,

}

impl Default for Engine {
    fn default() -> Self {
        let res = Self {
            associated_functions: Default::default(),
            functions: Default::default(),
            built_in_associated_functions: Default::default(),
            built_in_functions: Default::default(),
            binary_operators: impl_operators::get_binary_operators().into_iter()
                .map(|(name, function)| {
                    (name.to_string(), FunctionInfo::new(function).inline())
                })
                .collect(),
            unary_operators: impl_operators::get_unary_operators().into_iter()
                .map(|(name, function)| {
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
        #[cfg(feature = "std")]
        let mut res = res;
        #[cfg(feature = "std")]
        res.add_function(FunctionDefinition::new("print", |value: String| {
            println!("{value}");
        }));
        #[cfg(feature = "std")]
        res.add_function(FunctionDefinition::new("println", |value: String| {
            println!("{value}");
        }));
        res
    }
}


impl Engine {
    pub fn add_constant<Name: ToString, Value: Into<VBValue>>(&mut self, name: Name, value: Value) -> Option<VBValue> {
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

    pub fn parse<'input>(&self, input: &'input str, context_builder: ContextBuilder) -> Result<AST, ParsingError<'input>> {
        let successful_parse = SimpleParser::parse(Rule::BASE_STATEMENTS, input)
            .map_err(|e| ParsingError::Parsing(e))?
            .next().unwrap();
        parsing::build_ast(successful_parse.clone(), self, context_builder)
            .map_err(|errors| {
                let mut error = "Could not compile.".to_string().to_simple_error();
                errors.into_iter().for_each(|ind_error| { error.add_cause(ind_error) });
                ParsingError::CouldntBuildAST(error)
            })
    }

    pub(crate) fn find_unary_operator(&self, operator_name: &str) -> Option<&FunctionInfo> {
        self.unary_operators.get(operator_name)
    }

    pub(crate) fn find_binary_operator(&self, operator_name: &str) -> Option<&FunctionInfo> {
        self.binary_operators.get(operator_name)
    }

    pub(crate) fn find_function(&self, type_name: Option<String>, module_name: Option<&str>, function_name: &str) -> Option<&FunctionInfo> {
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
    pub(crate) fn constants(&self) -> &HashMap<String, VBValue> {
        &self.constants
    }
    pub(crate) fn binary_operation_parser(&self) -> &PrattParser<Rule> {
        &self.binary_operation_parser
    }
}
