use alloc::fmt::Debug;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::mem;

use pest::Parser;

use crate::execution::ASTFunction;
use crate::function::ToAbstractFunction;
use crate::parsing::{Rule, SimpleParser};
use crate::value::{FullValue, MoonValue};
use crate::MoonValueKind;

/// Configures an Engine about a specific script to compile, this includes giving input variables
#[derive(Debug, Clone)]
pub struct ContextBuilder {
    pub(crate) in_use_variables: Vec<(usize, Vec<InputVariable>)>,
    pub(crate) past_variables: Vec<(usize, Vec<InputVariable>)>,
    pub(crate) next_block_level: usize,
    pub(crate) started_parsing: bool,
    pub(crate) start_parsing_position_offset: (usize, usize),
    pub(crate) parsing_position_column_is_fixed: bool,
}

impl AsRef<ContextBuilder> for ContextBuilder {
    fn as_ref(&self) -> &ContextBuilder {
        self
    }
}

impl Default for ContextBuilder {
    fn default() -> Self {
        let mut res = Self {
            in_use_variables: vec![],
            past_variables: vec![],
            next_block_level: 0,
            started_parsing: false,
            start_parsing_position_offset: (0, 0),
            parsing_position_column_is_fixed: false,
        };
        res.push_block_level();
        res
    }
}

impl ContextBuilder {
    /// Creates a new empty ContextBuilder
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn current_depth(&self) -> usize {
        self.in_use_variables.len()
    }

    pub(crate) fn push_block_level(&mut self) {
        self.in_use_variables.push((self.next_block_level, Vec::new()));
        self.next_block_level += 1;
    }

    pub(crate) fn pop_block_level(&mut self) {
        let last_depth = self.in_use_variables.remove(self.in_use_variables.len() - 1);
        if !last_depth.1.is_empty() {
            self.past_variables.push(last_depth);
        }

        let current_depth = self.current_depth();
        self.in_use_variables.iter_mut().flat_map(|(_, v)| v)
            .for_each(|variable| {
                if current_depth < variable.type_is_valid_up_to_depth {
                    variable.associated_type_name = None;
                }
                if current_depth < variable.value_is_valid_up_to_depth {
                    variable.current_known_value = None;
                }
            });
    }

    pub(crate) fn take_all_variables(&mut self) -> Vec<(usize, Vec<InputVariable>)> {
        let mut variables = mem::take(&mut self.in_use_variables);
        variables.extend(mem::take(&mut self.past_variables));
        variables
    }

    pub(crate) fn find_variable(&mut self, variable_name: &str) -> Option<(usize, usize, &mut InputVariable)> {
        self.in_use_variables.iter_mut().rev()
            .map(|(block_level, var)|
                (*block_level, var.iter_mut().enumerate().rev().filter(|(_, var)| var.name.eq(variable_name)).next())
            )
            .filter(|(_, var)| var.is_some())
            .next()
            .map(|(index, v)| v.map(|(var_index, var)| (index, var_index, var)))
            .flatten()
    }

    /// Pushes an input variable, if said it's constant, the AST will have its value inlined
    pub fn push_variable<Variable: Into<InputVariable>>(&mut self, variable: Variable) -> (usize, usize) {
        self.push_variable_internal(variable, false)
    }

    /// Adds an input variable, if said it's constant, the AST will have its value inlined
    pub fn with_variable<Variable: Into<InputVariable>>(mut self, variable: Variable) -> ContextBuilder {
        self.push_variable_internal(variable, false);
        self
    }

    /// Specifies an starting position for this script, this is useful if you use a file with
    /// multiple scripts for managing error.
    ///
    /// Example: If you had a file containing multiples scripts, and one was located on the line 100
    /// and had a syntax / AST Parsing on it's 3rd line, then the error will tell the error happens
    /// on line 103, instead of line 3.
    pub fn start_parsing_position_offset(&mut self, line_offset: usize, column_offset: usize) {
        self.start_parsing_position_offset = (line_offset, column_offset)
    }

    /// Specifies an starting position for this script, this is useful if you use a file with
    /// multiple scripts for managing error.
    ///
    /// Example: If you had a file containing multiples scripts, and one was located on the line 100
    /// and had a syntax / AST Parsing on it's 3rd line, then the error will tell the error happens
    /// on line 103, instead of line 3.
    pub fn with_start_parsing_position_offset(mut self, line_offset: usize, column_offset: usize) -> ContextBuilder {
        self.start_parsing_position_offset = (line_offset, column_offset);
        self
    }

    /// Specifies it the column indicated in [Self::with_start_parsing_position_offset] is a fixed
    /// one or not.
    ///
    /// Example: If you had a file containing a script, but it is idented by 4 spaces on every line,
    /// setting this to true and setting the column offset as 4 would make that errors happening
    /// on colum 4 would appear as they happened on column 0 instead, so if the error its on column
    /// 100, it would say the error starts at 96 instead.
    pub fn parsing_column_fixed(&mut self, parsing_position_column_is_fixed: bool) {
        self.parsing_position_column_is_fixed = parsing_position_column_is_fixed
    }

    /// Specifies it the column indicated in [Self::with_start_parsing_position_offset] is a fixed
    /// one or not.
    ///
    /// Example: If you had a file containing a script, but it is idented by 4 spaces on every line,
    /// setting this to true and setting the column offset as 4 would make that errors happening
    /// on colum 4 would appear as they happened on column 0 instead, so if the error its on column
    /// 100, it would say the error starts at 96 instead.
    pub fn with_parsing_column_fixed(mut self, parsing_position_column_is_fixed: bool) -> ContextBuilder {
        self.parsing_position_column_is_fixed = parsing_position_column_is_fixed;
        self
    }


    pub(crate) fn push_variable_internal<Variable: Into<InputVariable>>(&mut self, variable: Variable, declare_variable_as_new: bool) -> (usize, usize) {
        let mut variable = variable.into();
        if !declare_variable_as_new {
            let already_existing_variable_index = self.in_use_variables[0].1.iter().position(|int_variable| int_variable.name.eq(&variable.name));
            return if let Some(already_existing_variable_index) = already_existing_variable_index {
                let current_depth = self.current_depth();
                let int_variable = &mut self.in_use_variables[0].1[already_existing_variable_index];
                if !int_variable.associated_type_name.eq(&variable.associated_type_name) {
                    variable.type_is_valid_up_to_depth = current_depth;
                    variable.value_is_valid_up_to_depth = current_depth;
                }
                if !int_variable.current_known_value.eq(&variable.current_known_value) {
                    variable.value_is_valid_up_to_depth = current_depth;
                }
                int_variable.current_known_value = variable.current_known_value;
                (0, already_existing_variable_index)
            } else {
                self.in_use_variables[0].1.push(variable);
                (0, self.in_use_variables[0].1.len() - 1)
            };
        }
        let last_block = self.in_use_variables.len() - 1;
        self.in_use_variables[last_block].1.push(variable);
        (self.in_use_variables[last_block].0, self.in_use_variables[last_block].1.len() - 1)
    }

    pub(crate) fn get_variable_at(&mut self, block_level: usize, var_index: usize) -> Option<&mut InputVariable> {
        self.in_use_variables.iter_mut()
            .filter(|(int_block_level, _)| block_level.eq(int_block_level))
            .map(|(_, block_variables)| block_variables.get_mut(var_index))
            .next().flatten()
    }
}

/// Holds information about a variable that can be given to a [crate::ContextBuilder] as a means of
/// inputs.
#[derive(Debug, Clone)]
pub struct InputVariable {
    pub(crate) name: String,
    pub(crate) first_value: FullValue,
    pub(crate) associated_type_name: Option<String>,
    pub(crate) current_known_value: Option<FullValue>,
    pub(crate) type_is_valid_up_to_depth: usize,
    pub(crate) value_is_valid_up_to_depth: usize,
    pub(crate) can_inline: bool,
}


impl InputVariable {
    pub(crate) fn inlineable_value(&mut self) -> Option<FullValue> {
        match self.can_inline{
            true => self.current_known_value.clone(),
            false => None
        }
    }

    /// Creates a new variable as a place-holder with a name, if it's value is indicated with
    /// [Self::value], it will turn into a constant variable, where it's value can be inlined when
    /// compiling an AST, the value can also be given with [Self::lazy_value] as part of a context,
    /// but if the value isn't given in none of these ways and its used in the script, you should
    /// give this value to the AST's executor thorough [crate::ASTExecutor::push_variable] or
    /// [crate::OptimizedASTExecutor::push_variable].
    pub fn new<Name: ToString>(name: Name) -> Self {
        let mut name = name.to_string();
        let parsed = SimpleParser::parse(Rule::ident, &*name);
        if parsed.is_err() || parsed.unwrap().as_str().len() < name.len() {
            name = "Wrong variable name".to_string();
        }
        Self {
            name,
            first_value: FullValue::Null,
            associated_type_name: None,
            current_known_value: None,
            value_is_valid_up_to_depth: 0,
            type_is_valid_up_to_depth: 0,
            can_inline: true,
        }
    }

    /// Specifies the value of this variable, this means it turns into a constant variable, and
    /// therefore it can be inlined when parsing an AST.
    pub fn value<Value: Into<MoonValue>>(mut self, value: Value) -> Self {
        let value = value.into();
        if self.associated_type_name.is_none(){
            self = self.associated_type_of::<Value>();
        }
        self.current_known_value = Some(FullValue::from(value));
        self
    }

    /// Specifies a function that gives the value of this variable, the difference between this and
    /// given it's value to the AST's executor is just performance, as
    /// [crate::ASTExecutor::push_variable] it's slower due to the need of checking a HashMap.
    pub fn lazy_value<Dummy, ReturnT: Into<MoonValue>, Function, AbstractFunction: ToAbstractFunction<(), ReturnT, Function, Dummy> + Clone>
    (mut self, function: AbstractFunction) -> Self {
        if self.associated_type_name.is_none(){
            self = self.associated_type_of::<ReturnT>();
        }
        self.first_value = FullValue::Function(ASTFunction { function: function.abstract_function(), args: Vec::new() });
        self.current_known_value = Some(self.first_value.clone());
        self
    }

    /// Specifies what kind type is associated to this variable, see the Properties section of the
    /// book for more information about properties.
    pub fn associated_type<'input, Name: Into<MoonValueKind<'input>>>(mut self, name: Name) -> Self {
        if let Some(name) = name.into().get_moon_value_type() {
            let parsed = SimpleParser::parse(Rule::ident, name);
            if parsed.is_err() || parsed.unwrap().as_str().len() < name.len() {
                return self;
            }
            self.associated_type_name = Some(name.to_string());
        }
        self
    }

    /// Specifies what kind type is associated to this variable, but instead of receiving a name or
    /// a [crate::MoonValueKind], it receives the value's type, this is preferred over
    /// [Self::associated_type] but it doesn't allow you to create pseudo-types, requiring the use
    /// of real types.
    ///
    /// see the Properties section of the book for more information about properties.
    pub fn associated_type_of<T>(mut self) -> Self {
        self.associated_type_name = MoonValueKind::get_kind_string_of::<T>();
        self
    }
}

