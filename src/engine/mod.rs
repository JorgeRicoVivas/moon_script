use alloc::string::{String, ToString};
use log::trace;
use pest::Parser;
use simple_detailed_error::SimpleErrorDetail;

use context::ContextBuilder;

use crate::execution::ast::AST;
use crate::parsing::error::ParsingError;
use crate::parsing::{FunctionDefinition, FunctionInfo, Rule, SimpleParser};
use crate::reduced_value_impl::impl_operators;
use crate::value::MoonValue;
use crate::{parsing, HashMap, MoonValueKind};


pub mod context;

#[derive(Clone)]
/// Scripting engine, it allows to create runnable ASTs, and also to give functions and constant
/// values for said scripts
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

    constants: HashMap<String, Constant>,
}

/// Defines a constant that will be inlined on scripts.
#[derive(Clone)]
pub struct Constant {
    pub(crate) value:MoonValue,
    pub(crate) type_name:Option<String>,
}

impl Constant{

    /// Creates a new constant out of this value by mapping it into a MoonValue
    pub fn new<Value: Into<MoonValue>>(value:Value) -> Constant {
        Constant::from(value.into())
    }

    /// Specifies what kind type is associated to this constant, see the Properties section of the
    /// book for more information about properties.
    pub fn associated_type<'input, Name: Into<MoonValueKind<'input>>>(mut self, name: Name) -> Self {
        self.type_name = name.into().get_moon_value_type().map(|string|string.to_string());
        self
    }

    /// Specifies what kind type is associated to this constant, but instead of receiving a name or
    /// a [crate::MoonValueKind], it receives the value's type, this is preferred over
    /// [Self::associated_type] but it doesn't allow you to create pseudo-types, requiring the use
    /// of real types.
    ///
    /// see the Properties section of the book for more information about properties.
    pub fn associated_type_of<T>(mut self) -> Self {
        self.type_name = MoonValueKind::get_kind_string_of::<T>();
        self
    }

    /// Gets the MoonValue associated to this constant
    pub fn value(&self) -> &MoonValue {
        &self.value
    }
}

impl<T:Into<MoonValue>> From<T> for Constant{
    fn from(value: T) -> Self {
        Self{
            type_name: MoonValueKind::get_kind_string_of::<T>(),
            value: value.into(),
        }
    }
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
    /// Creates a new empty Engine containing just basic functions, like println or binary operators
    pub fn new() -> Self {
        Default::default()
    }

    /// Adds a constant with a value
    ///
    /// ```rust
    /// use moon_script::{ContextBuilder, Engine};
    /// let mut engine = Engine::new();
    /// engine.add_constant("MY_CONSTANT", 15);
    /// let runnable_ast = engine.parse(r###"
    ///     return MY_CONSTANT;
    /// "###, ContextBuilder::default()).unwrap();
    /// let result : i32 = runnable_ast.executor().execute().unwrap().try_into().unwrap();
    /// assert_eq!(15, result);
    /// ```
    pub fn add_constant<Name: ToString, Value: Into<Constant>>(&mut self, name: Name, value: Value) -> Option<Constant> {
        self.constants.insert(name.to_string(), value.into())
    }

    /// Adds a function with a name
    ///
    /// ```rust
    /// use moon_script::{ContextBuilder, Engine, FunctionDefinition};
    /// let mut engine = Engine::new();
    /// engine.add_function(FunctionDefinition::new("say_hi_and_return_number", ||{
    ///     println!("Hi!");
    ///     return 5;
    /// }));
    /// let runnable_ast = engine.parse(r###"
    ///     return say_hi_and_return_number();
    /// "###, ContextBuilder::default()).unwrap();
    /// let result : i32 = runnable_ast.executor().execute().unwrap().try_into().unwrap();
    /// assert_eq!(5, result);
    /// ```
    pub fn add_function<Function: Into<FunctionDefinition>>(&mut self, function_definition: Function) {
        let function_definition = function_definition.into();
        trace!("Adding function: {function_definition:?}");
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

    /// Parses a script into an AST using a specific context
    ///
    /// Adds a function with a name
    ///
    /// ```rust
    /// use moon_script::{InputVariable, ContextBuilder, Engine, FunctionDefinition};
    ///
    /// let mut engine = Engine::new();
    /// engine.add_function(FunctionDefinition::new("add_five", |num:u8| {
    ///     return num + 5;
    /// }));
    ///
    /// let context = ContextBuilder::new()
    ///     .with_variable(InputVariable::new("ten").value(10));
    ///
    ///
    /// let runnable_ast = engine.parse(r###"
    ///     return add_five(ten);
    /// "###, context).unwrap();
    /// let result : i32 = runnable_ast.executor().execute().unwrap().try_into().unwrap();
    /// assert_eq!(15, result);
    /// ```
    pub fn parse<'input>(&self, input: &'input str, context_builder: ContextBuilder) -> Result<AST, ParsingError<'input>> {
        let successful_parse = SimpleParser::parse(Rule::BASE_STATEMENTS, input)
            .map_err(|e| ParsingError::Grammar(e))?
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
    pub(crate) fn constants(&self) -> &HashMap<String, Constant> {
        &self.constants
    }

}
