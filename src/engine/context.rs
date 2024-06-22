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

#[derive(Debug, Clone)]
pub struct ContextBuilder {
    pub(crate) in_use_variables: Vec<(usize, Vec<CompiletimeVariableInformation>)>,
    pub(crate) past_variables: Vec<(usize, Vec<CompiletimeVariableInformation>)>,
    pub(crate) next_block_level: usize,
    pub(crate) started_parsing: bool,
    pub(crate) variables_should_inline: bool,
    pub(crate) start_parsing_position_offset: (usize, usize),
    pub(crate) parsing_position_column_is_fixed: bool,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        let mut res = Self {
            in_use_variables: vec![],
            past_variables: vec![],
            next_block_level: 0,
            started_parsing: false,
            variables_should_inline: true,
            start_parsing_position_offset: (0, 0),
            parsing_position_column_is_fixed: false,
        };
        res.push_block_level();
        res
    }
}

impl ContextBuilder {
    pub(crate) fn current_depth(&self) -> usize {
        self.in_use_variables.len()
    }

    pub(crate) fn forbid_variables_from_inlining(&mut self) {
        self.in_use_variables.iter_mut().flat_map(|(_, v)| v)
            .for_each(|variable| { variable.global_can_inline = false });
        self.variables_should_inline = true;
    }

    pub(crate) fn permit_variables_to_inline(&mut self) {
        self.in_use_variables.iter_mut().flat_map(|(_, v)| v)
            .for_each(|variable| { variable.global_can_inline = true });
        self.variables_should_inline = false;
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

    pub(crate) fn take_all_variables(&mut self) -> Vec<(usize, Vec<CompiletimeVariableInformation>)> {
        let mut variables = mem::take(&mut self.in_use_variables);
        variables.extend(mem::take(&mut self.past_variables));
        variables
    }

    pub(crate) fn find_variable(&mut self, variable_name: &str) -> Option<(usize, usize, &mut CompiletimeVariableInformation)> {
        self.in_use_variables.iter_mut().rev()
            .map(|(block_level, var)|
                (*block_level, var.iter_mut().enumerate().rev().filter(|(_, var)| var.name.eq(variable_name)).next())
            )
            .filter(|(_, var)| var.is_some())
            .next()
            .map(|(index, v)| v.map(|(var_index, var)| (index, var_index, var)))
            .flatten()
    }

    pub fn push_variable<Variable: Into<CompiletimeVariableInformation>>(&mut self, variable: Variable) -> (usize, usize) {
        self.push_variable_internal(variable, false)
    }

    pub fn with_variable<Variable: Into<CompiletimeVariableInformation>>(mut self, variable: Variable) -> ContextBuilder {
        self.push_variable_internal(variable, false);
        self
    }

    pub fn start_parsing_position_offset(&mut self, line_offset: usize, column_offset: usize) {
        self.start_parsing_position_offset = (line_offset, column_offset)
    }

    pub fn with_start_parsing_position_offset(mut self, line_offset: usize, column_offset: usize) -> ContextBuilder {
        self.start_parsing_position_offset = (line_offset, column_offset);
        self
    }

    pub fn parsing_column_fixed(&mut self, parsing_position_column_is_fixed: bool) {
        self.parsing_position_column_is_fixed = parsing_position_column_is_fixed
    }

    pub fn with_parsing_column_fixed(mut self, parsing_position_column_is_fixed: bool) -> ContextBuilder {
        self.parsing_position_column_is_fixed = parsing_position_column_is_fixed;
        self
    }


    pub(crate) fn push_variable_internal<Variable: Into<CompiletimeVariableInformation>>(&mut self, variable: Variable, declare_variable_as_new: bool) -> (usize, usize) {
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

    pub(crate) fn get_variable_at(&mut self, block_level: usize, var_index: usize) -> Option<&mut CompiletimeVariableInformation> {
        self.in_use_variables.iter_mut()
            .filter(|(int_block_level, _)| block_level.eq(int_block_level))
            .map(|(_, block_variables)| block_variables.get_mut(var_index))
            .next().flatten()
    }
}


#[derive(Debug, Clone)]
pub struct CompiletimeVariableInformation {
    pub(crate) name: String,
    pub(crate) first_value: FullValue,
    pub(crate) associated_type_name: Option<String>,
    pub(crate) current_known_value: Option<FullValue>,
    pub(crate) type_is_valid_up_to_depth: usize,
    pub(crate) value_is_valid_up_to_depth: usize,
    pub(crate) can_inline: bool,
    pub(crate) global_can_inline: bool,
}


impl CompiletimeVariableInformation {
    pub(crate) fn inlineable_value(&mut self) -> Option<FullValue> {
        if self.can_inline && self.global_can_inline {
            self.current_known_value.clone()
        } else {
            self.can_inline = false;
            None
        }
    }

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
            global_can_inline: true,
        }
    }

    pub fn value<Value: Into<MoonValue>>(mut self, value: Value) -> Self {
        let value = value.into();
        if self.associated_type_name.is_none() {
            self.associated_type_name = Some(value.type_name().to_string());
        }
        self.current_known_value = Some(FullValue::from(value));
        self
    }

    pub fn lazy_value<Dummy, ReturnT: Into<MoonValue>, Function, AbstractFunction: ToAbstractFunction<(), ReturnT, Function, Dummy> + Clone>
    (mut self, function: AbstractFunction) -> Self {
        self.first_value = FullValue::Function(ASTFunction { function: function.clone().abstract_function(), args: Vec::new() });
        self.current_known_value = Some(FullValue::Function(ASTFunction { function: function.abstract_function(), args: Vec::new() }));
        self
    }

    pub fn associated_type<Name: ToString>(mut self, name: Name) -> Self {
        let name = name.to_string();
        let parsed = SimpleParser::parse(Rule::ident, &*name);
        if parsed.is_err() || parsed.unwrap().as_str().len() < name.len() {
            return self;
        }
        self.associated_type_name = Some(name);
        self
    }
}
