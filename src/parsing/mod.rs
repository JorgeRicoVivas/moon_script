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
use crate::execution::ast::{Statement, AST};
use crate::execution::RuntimeVariable;
use crate::function::{MoonFunction, ToAbstractFunction};
use crate::value::FullValue;
use crate::HashMap;
use crate::HashSet;
use crate::LazyLock;


pub(crate) mod value_parsing;
pub(crate) mod statement_parsing;
pub mod error;

#[derive(Parser)]
#[grammar = "language_definition.pest"]
pub(crate) struct SimpleParser;

#[derive(Clone, Debug)]
pub(crate) struct FunctionInfo {
    can_inline_result: bool,
    function: MoonFunction,
    return_type_name: Option<String>,
}

impl FunctionInfo {
    pub(crate) fn new<Dummy, Params, ReturnValue, Function, AbstractFunction: ToAbstractFunction<Params, ReturnValue, Function, Dummy>>
    (function: AbstractFunction) -> Self {
        Self::new_raw(function.abstract_function())
    }

    pub(crate) const fn new_raw(function: MoonFunction) -> Self {
        Self { function, return_type_name: None, can_inline_result: false }
    }

    pub(crate) const fn inline(mut self) -> FunctionInfo {
        self.can_inline_result = true;
        self
    }
}

/// Builder pattern for defining custom Engine's functions
#[derive(Clone, Debug)]
pub struct FunctionDefinition {
    pub(crate) associated_type_name: Option<String>,
    pub(crate) module_name: Option<String>,
    pub(crate) function_name: String,
    pub(crate) function_info: FunctionInfo,
}

impl FunctionDefinition {
    /// Creates a new function with the name and function indicated as arguments.
    ///
    /// This for example defines a function called 'sum_two' that sums two **u8**s:
    ///
    /// ```rust
    ///moon_script::FunctionDefinition::new("sum_two", |num:u8, other:u8| num+other);
    /// ```
    ///
    /// You can use it this way:
    ///
    /// ```rust
    /// use moon_script::{ContextBuilder, Engine, FunctionDefinition};
    ///
    /// let my_sum_function = FunctionDefinition::new("sum_two", |num:u8, other:u8| num+other);
    ///
    /// let mut engine = Engine::new();
    /// engine.add_function(my_sum_function);
    ///
    /// let result = engine.parse("return sum_two(10,5);", ContextBuilder::new())
    ///     .unwrap().execute().map(|value|u8::try_from(value)).unwrap().unwrap();
    ///
    /// assert_eq!(15, result);
    /// ```
    pub fn new<Name: Into<String>, Dummy, Params, ReturnValue, Function, AbstractFunction: ToAbstractFunction<Params, ReturnValue, Function, Dummy>>
    (function_name: Name, function: AbstractFunction) -> Self {
        let mut function_info = FunctionInfo::new_raw(function.abstract_function());
        function_info.return_type_name = MoonValueKind::get_kind_string_of::<ReturnValue>();
        Self {
            function_info: function_info,
            function_name: function_name.into(),
            module_name: None,
            associated_type_name: None,
        }
    }

    /// Specifies the module name for this function.
    pub fn module_name<Name: Into<String>>(mut self, module_name: Name) -> Self {
        self.module_name = Some(module_name.into());
        self
    }

    /// Specifies the associated type for this function.
    ///
    ///
    /// This is a function associated to the moon script primitive Integer:
    /// ```rust
    /// moon_script::FunctionDefinition::new("sum_two", |num:u8, other:u8| num+other)
    ///     .associated_type_name(moon_script::MoonValueKind::Integer);
    /// ```
    ///
    /// This is a function associated to a custom type that can be read as an u8:
    /// ```rust
    /// moon_script::FunctionDefinition::new("sum_two", |num:u8, other:u8| num+other)
    ///     .associated_type_name("MyCustomTypeName");
    /// ```
    ///
    pub fn associated_type_name<'input, Name: Into<MoonValueKind<'input>>>(mut self, associated_type_name: Name) -> Self {
        self.associated_type_name = associated_type_name.into().get_moon_value_type().map(|string| string.to_string());
        self
    }

    /// Specifies the associated type for this function, but instead of receiving a name or a
    /// [crate::MoonValueKind], it receives the value, this is preferred over
    /// [Self::associated_type_name] but it doesn't allow you to create pseudo-types, requiring
    /// the use of real types.
    ///
    /// ```
    /// use moon_script::{ContextBuilder, Engine, FunctionDefinition, InputVariable};
    /// let mut engine = Engine::new();
    /// engine.add_function(FunctionDefinition::new("add_two", |n:u8|n+2).associated_type_of::<u8>());
    /// let context_with_variable = ContextBuilder::new().with_variable(InputVariable::new("five").associated_type_of::<u8>());
    /// let ast = engine.parse("five.add_two()", context_with_variable).unwrap();
    ///
    /// let result : u8 = ast.executor().push_variable("five", 5).execute().unwrap().try_into().unwrap();
    ///
    /// assert_eq!(7, result);
    ///
    /// ```
    pub fn associated_type_of<T>(mut self) -> Self {
        self.associated_type_name = MoonValueKind::get_kind_string_of::<T>();
        self
    }

    /// Marks this function as constant, being able to inline it's results when compiling the script
    /// if the arguments are also constant.
    pub const fn inline(mut self) -> Self {
        self.function_info.can_inline_result = true;
        self
    }

    /// Specifies the type of the return value for this function, if let unmarked, associations
    /// cannot be used and therefore properties won't work.
    pub fn known_return_type_name<'input, Name: Into<MoonValueKind<'input>>>(mut self, return_type_name: Name) -> Self {
        self.function_info.return_type_name = return_type_name.into().get_moon_value_type().map(|string| string.to_string());
        self
    }

    /// Specifies the type of the return value for this function, but instead of receiving a name or
    /// a [crate::MoonValueKind], it receives the value, this is preferred over
    /// [Self::known_return_type_name] but it doesn't allow you to create pseudo-types, requiring
    /// the use of real types.
    pub fn known_return_type_of<T>(mut self) -> Self {
        self.function_info.return_type_name = MoonValueKind::get_kind_string_of::<T>();
        self
    }
}


struct Privatize;

/// Types of Moon values
pub enum MoonValueKind<'selflf> {
    Null,
    Boolean,
    Integer,
    Decimal,
    String,
    Array,
    Function,
    Invalid,
    #[allow(private_interfaces)]
    CustomStr(&'selflf str, Privatize),
    #[allow(private_interfaces)]
    CustomString(String, Privatize),
}

static RESERVED_MOON_VALUE_KINDS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    [MoonValueKind::Null, MoonValueKind::Boolean, MoonValueKind::Integer,
        MoonValueKind::Decimal, MoonValueKind::String, MoonValueKind::Array,
        MoonValueKind::Function]
        .map(|value_kind| value_kind.get_moon_value_type().unwrap().to_string())
        .into_iter()
        .collect::<HashSet<String>>()
});

pub(crate) static RUST_TYPES_TO_MOON_VALUE_KINDS: LazyLock<HashMap<&'static str, String>> = LazyLock::new(|| {
    [
        (core::any::type_name::<()>(), MoonValueKind::Null),
        (core::any::type_name::<bool>(), MoonValueKind::Boolean),
        (core::any::type_name::<i8>(), MoonValueKind::Integer),
        (core::any::type_name::<i16>(), MoonValueKind::Integer),
        (core::any::type_name::<i32>(), MoonValueKind::Integer),
        (core::any::type_name::<i64>(), MoonValueKind::Integer),
        (core::any::type_name::<i128>(), MoonValueKind::Integer),
        (core::any::type_name::<isize>(), MoonValueKind::Integer),
        (core::any::type_name::<u8>(), MoonValueKind::Integer),
        (core::any::type_name::<u16>(), MoonValueKind::Integer),
        (core::any::type_name::<u32>(), MoonValueKind::Integer),
        (core::any::type_name::<u64>(), MoonValueKind::Integer),
        (core::any::type_name::<u128>(), MoonValueKind::Integer),
        (core::any::type_name::<usize>(), MoonValueKind::Integer),
        (core::any::type_name::<f32>(), MoonValueKind::Decimal),
        (core::any::type_name::<f64>(), MoonValueKind::Decimal),
        (core::any::type_name::<String>(), MoonValueKind::String),
    ]
        .map(|(rust_type, moon_value_kind)| {
            (rust_type, moon_value_kind.get_moon_value_type().unwrap().to_string())
        })
        .into_iter()
        .collect()
});


static RESULT_TYPE_PREFIX: LazyLock<String> = LazyLock::new(|| {
    let result = core::any::type_name::<Result<(), ()>>();
    let result_start = result.find(r"<").unwrap();
    result[0..result_start + 1].to_string()
});

fn decouple_ok_argument_from_its_result(type_in_use: &str) -> Option<&str> {
    if !type_in_use.starts_with(&*RESULT_TYPE_PREFIX) { return None; };

    let type_in_use = &type_in_use[RESULT_TYPE_PREFIX.len()..];
    let mut opened_brackets_and_diamonds = 0_usize;
    let end = type_in_use.chars().enumerate().filter(|(_, char)| {
        match char {
            '(' | '<' => opened_brackets_and_diamonds += 1,
            ')' | '>' => opened_brackets_and_diamonds -= 1,
            ',' => return true,
            _ => {}
        }
        false
    }).next().unwrap().0;
    Some(&type_in_use[..end])
}

impl MoonValueKind<'_> {
    pub(crate) fn get_kind_string_of<T>() -> Option<String> {
        RUST_TYPES_TO_MOON_VALUE_KINDS
            .get(core::any::type_name::<T>()).cloned()
            .map(|string|
                decouple_ok_argument_from_its_result(&string).map(|s| s.to_string()).unwrap_or(string)
            )
            .or_else(||
                MoonValueKind::from(core::any::type_name::<T>())
                    .get_moon_value_type().map(|string| string.to_string())
            )
            .filter(|name| !name.eq("null"))
    }

    pub(crate) fn get_moon_value_type(&self) -> Option<&str> {
        Some(match self {
            MoonValueKind::Null => "null",
            MoonValueKind::Boolean => "bool",
            MoonValueKind::Integer => "int",
            MoonValueKind::Decimal => "decimal",
            MoonValueKind::String => "string",
            MoonValueKind::Array => "array",
            MoonValueKind::Function => "function",
            MoonValueKind::Invalid => return None,
            MoonValueKind::CustomStr(str, _) => str,
            MoonValueKind::CustomString(str, _) => str
        })
    }
}

impl<'typename> From<&'typename str> for MoonValueKind<'typename> {
    fn from(value: &'typename str) -> Self {
        if RESERVED_MOON_VALUE_KINDS.contains(value) {
            return Self::Invalid;
        }
        Self::CustomStr(value, Privatize)
    }
}

impl From<String> for MoonValueKind<'_> {
    fn from(value: String) -> Self {
        if RESERVED_MOON_VALUE_KINDS.contains(&value) {
            return Self::Invalid;
        }
        Self::CustomString(value, Privatize)
    }
}

fn optimize_variables(context: &mut ContextBuilder, inlineable_variables: Vec<(String, usize)>, statements: &mut Vec<Statement>) -> (Vec<RuntimeVariable>, HashMap<String, usize>) {
    let variables = context.take_all_variables();
    let mut variables = variables.into_iter()
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

pub(crate) fn build_ast<'input>(token: Pair<'input, Rule>, base: &Engine, mut context: ContextBuilder) -> Result<AST, Vec<SimpleError<'input>>> {
    if token.as_rule() != Rule::BASE_STATEMENTS {}
    let statements_tokens = token.into_inner().next().unwrap();
    context.started_parsing = true;
    let inlineable_variables = context.in_use_variables.get(0).map(|(_, variables)| {
        variables.iter().enumerate()
            .filter(|(_, variable)| { variable.current_known_value.is_none() })
            .map(|(block_0_var_index, variable)| (variable.name.clone(), block_0_var_index))
            .collect::<Vec<_>>()
    }).unwrap_or_default();
    let mut statements = statement_parsing::build_token(statements_tokens, base, &mut context, true)?;
    replace_last_fn_call_for_return_statement(&mut statements);

    let (variables, parameterized_variables) = optimize_variables(&mut context, inlineable_variables, &mut statements);
    Ok(AST { statements, variables, parameterized_variables })
}

fn replace_last_fn_call_for_return_statement(statements: &mut Vec<Statement>) {
    if let Some(last_statement) = statements.last_mut() {
        let is_fn_call = match last_statement {
            Statement::FnCall(_) => true,
            _ => false,
        };
        if is_fn_call {
            let fn_call = match mem::replace(last_statement, Statement::ReturnCall(FullValue::Null)) {
                Statement::FnCall(function) => function,
                _ => unreachable!()
            };
            *last_statement = Statement::ReturnCall(FullValue::Function(fn_call));
        }
    }
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