use alloc::fmt::{Debug, Display, Formatter};
use alloc::format;
use alloc::string::String;

use pest::error::LineColLocation;
use simple_detailed_error::{SimpleError, SimpleErrorDetail, SimpleErrorExplanation};

use crate::execution::RuntimeError;
use crate::parsing::Rule;

#[cfg(feature = "colorization")]
use alloc::vec::Vec;
#[cfg(feature = "colorization")]
use colored::Colorize;
#[cfg(feature = "colorization")]
use string_colorization::{foreground, style};


/// Error happened while parsing, this can happen due to a grammar parsing error, or if the syntax
/// it's right, because the of a series of [ASTBuildingError].
#[derive(Debug)]
pub enum ParsingError<'input> {
    /// Happens if the script doesn't match Moon Script's grammar.
    Grammar(pest::error::Error<Rule>),
    /// Happens if the grammar is right, but at least one [ASTBuildingError] happens.
    ///
    /// Why isn't this a series of [ASTBuildingError]s?: Individual programs are extremely unlikely
    /// to manually use them internally, but rather want a clear output telling why the script is
    /// wrong, with [simple_detailed_error::SimpleError], Moon Script is able to specifies as many
    /// errors as possible using a clear tree structure, and errors are given with colors* when
    /// printing errors, showing with clarity where the error happens.
    ///
    /// * For accessibility, please, read the [colored] create used by [simple_detailed_error],
    /// which uses [NO_COLOR](https://no-color.org/).
    CouldntBuildAST(SimpleError<'input>),
}

impl<'input> ParsingError<'input> {
    pub fn couldnt_build_ast_error(self) -> Option<SimpleError<'input>> {
        match self{
            Self::CouldntBuildAST(error)=>Some(error),
            _=>None
        }
    }
}

impl<'input> Display for ParsingError<'input> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            ParsingError::Grammar(pest_error) => f.write_str(&format!("{pest_error}")),
            ParsingError::CouldntBuildAST(simple_error) => f.write_str(&format!("{}", simple_error.as_display_struct(true))),
        }
    }
}

impl<'input> From<ParsingError<'input>> for SimpleError<'input> {
    fn from(value: ParsingError<'input>) -> Self {
        match value {
            ParsingError::Grammar(parsing) => {
                let mut error = SimpleError::new()
                    .error_detail(format!("On {} because of {}\nDetail:{}", parsing.line(), parsing.variant, parsing));
                match parsing.line_col {
                    LineColLocation::Pos((start_line, start_col)) => {
                        error = error.start_point_of_error(start_line, start_col);
                    }
                    LineColLocation::Span((start_line, start_col), (end_line, end_col)) => {
                        error = error.start_point_of_error(start_line, start_col).end_point_of_error(end_line, end_col);
                    }
                }
                error
            }
            ParsingError::CouldntBuildAST(error) => error,
        }
    }
}

#[cfg(feature = "std")]
impl<'input> std::error::Error for ParsingError<'input> {}

/// Specifies why an AST could not be parsed, the 'input lifetime points references the input of
/// your script's String value
#[derive(Debug)]
pub enum ASTBuildingError<'input> {
    /// A constant predicate cannot be resolved, likely because of wrong constant function
    ConditionDoestNotResolveToBoolean {
        /// Predicate where it happens (This is a reference to the script that is tried to compile).
        predicate: &'input str
    },
    /// An ident of a variable was found, but the name doesn't match to a variable that was created
    /// inside the script, nor an Engine's constants or the ContextBuilder input variables
    VariableNotInScope {
        /// Name of the variable.
        variable_name: &'input str
    },
    /// Used an operator that doesn't exist, this will likely never happen
    OperatorNotFound {
        /// Name of the operator as symbol.
        operator: &'input str
    },
    /// A function name was specified, but said function doesn't exist on the Engine
    FunctionNotFound {
        /// Name of the function.
        function_name: &'input str,
        /// Associated type  (Might be none if it's not specified in the script).
        associated_to_type: Option<String>,
        /// Module (Might be none if it's not specified in the script).
        module: Option<&'input str>,
    },
    /// A property was specified, but it doesn't exist on the Engine (See the Properties section of
    /// the book for more information)
    PropertyFunctionNotFound {
        /// Preferred name of the property to find, this is set_*name* in setters and get_*name* in
        /// getters.
        preferred_property_to_find: String,
        /// Name of the function (Property).
        original_property: &'input str,
        /// Associated type of the variable (Might not have one if the variable type is not
        /// specified).
        typename: Option<String>,
    },
    /// An error was triggered while inlining a constant function
    CouldntInlineFunction {
        /// Name of the function
        function_name: &'input str,
        /// Specified the error that happened
        runtime_error: RuntimeError,
    },
    /// An error was triggered while inlining a constant getter (See getters in the Properties
    /// section of the book for more information about properties)
    CouldntInlineGetter {
        /// Explanation of the error
        execution_error_message: String,
        /// Name of the property
        property: &'input str,
    },
    /// An unary operator was tried to be inlined with a constant value, but said function failed
    CouldntInlineUnaryOperator {
        /// Symbol of the operator
        operator: &'input str,
        /// Specified the error that happened
        runtime_error: RuntimeError,
    },
    /// A binary operator was tried to be inlined with a constant value, but said function failed
    CouldntInlineBinaryOperator {
        /// Symbol of the operator
        operator: &'input str,
        /// Specified the error that happened
        runtime_error: RuntimeError,
    },
    /// Tried to inline a constant variable whose type wasn't specified in the ContextBuilder (nor
    /// the Engine if it is a constant).
    CouldntInlineVariableOfUnknownType {
        /// Name of the variable
        variable_name: &'input str
    },
    /// An integer value could not be parsed in range
    CannotParseInteger {
        /// Value (This is a reference to the script that is tried to compile).
        value: &'input str,
        /// Minimum bound the string should have been
        lower_bound: i128,
        /// Maximum bound the string should have been
        upper_bound: i128,
    },
    /// A decimal value could not be parsed in range
    CannotParseDecimal {
        /// Value (This is a reference to the script that is tried to compile).
        value: &'input str,
        /// Minimum bound the string should have been
        lower_bound: f64,
        /// Maximum bound the string should have been
        upper_bound: f64,
    },
}

#[cfg(not(feature = "colorization"))]
trait PseudoColored {
    fn green(&self) -> &Self { self }
    fn bold(&self) -> &Self { self }
    fn italic(&self) -> &Self { self }
}

#[cfg(not(feature = "colorization"))]
impl PseudoColored for str {}

impl<'input> SimpleErrorDetail for ASTBuildingError<'input> {
    fn explain_error(&self) -> SimpleErrorExplanation {
        let explanation;
        let mut solution = String::new();
        #[cfg(feature = "colorization")]
        let mut colorization_markers: Vec<(&str, string_colorization::Colorizer)> = Vec::new();
        match self {
            ASTBuildingError::ConditionDoestNotResolveToBoolean { predicate } => {
                explanation = format!("The predicate '{}' doesn't resolve to a boolean value", predicate.bold());
                #[cfg(feature = "colorization")]
                colorization_markers.push((predicate, style::Clear + foreground::Red));
            }
            ASTBuildingError::VariableNotInScope { variable_name } => {
                explanation = format!("The variable {} does not exist.", variable_name.bold());
                solution = format!("If this is a local variable, create it before using it, like:\nlet {} = *{}*", variable_name.green().bold(), "your value".italic());
                #[cfg(feature = "colorization")]
                colorization_markers.push((variable_name, style::Clear + foreground::Red));
            }
            ASTBuildingError::OperatorNotFound { operator } => {
                explanation = format!("The operator {} does not exist.", operator.bold());
                #[cfg(feature = "colorization")]
                colorization_markers.push((operator, style::Clear + foreground::Red));
            }
            ASTBuildingError::FunctionNotFound { function_name, module, associated_to_type } => {
                explanation = format!("There is no function {}{}{}.",
                                      function_name.bold(),
                                      module.as_ref().map(|module| format!(" in module {module}"))
                                          .unwrap_or_else(|| format!(" in any module")),
                                      associated_to_type.as_ref().map(|associated_type| format!(" for type {associated_type}")).unwrap_or_default()
                );
                #[cfg(feature = "colorization")]
                colorization_markers.push((function_name, style::Clear + foreground::Red));
            }
            ASTBuildingError::PropertyFunctionNotFound { preferred_property_to_find, original_property, typename } => {
                let typename = typename.as_ref().map(|v| &**v).unwrap_or("Unknown type");
                explanation = format!("The type {typename} does not have a property named {} as there is no associated function named {preferred_property_to_find} nor {original_property}.",
                                      original_property.bold()
                );
                #[cfg(feature = "colorization")]
                colorization_markers.push((original_property, style::Clear + foreground::Red));
            }
            ASTBuildingError::CouldntInlineFunction { function_name, runtime_error } => {
                explanation = format!("The constant function {} was tried to be inlined, but it returned this error:\n{}.", function_name.bold(), runtime_error.explain());
                #[cfg(feature = "colorization")]
                colorization_markers.push((function_name, style::Clear + foreground::Red));
            }
            ASTBuildingError::CouldntInlineGetter { property, execution_error_message } => {
                explanation = format!("The constant property getter {} was tried to be inlined, but it returned this error:\n{execution_error_message}.", property.bold());
                #[cfg(feature = "colorization")]
                colorization_markers.push((property, style::Clear + foreground::Red));
            }
            ASTBuildingError::CouldntInlineUnaryOperator { operator, runtime_error } => {
                explanation = format!("The constant operator {} was tried to be inlined, but it returned this error:\n{}.", operator.bold(), runtime_error.explain());
                #[cfg(feature = "colorization")]
                colorization_markers.push((operator, style::Clear + foreground::Red));
            }
            ASTBuildingError::CouldntInlineBinaryOperator { operator, runtime_error } => {
                explanation = format!("The constant binary operator {} was tried to be inlined, but it returned this error:\n{}.", operator.bold(), runtime_error.explain());
                #[cfg(feature = "colorization")]
                colorization_markers.push((operator, style::Clear + foreground::Red));
            }
            ASTBuildingError::CouldntInlineVariableOfUnknownType { variable_name } => {
                explanation = format!("Variable {} was tried to be inlined, but its type is unknown at this point.", variable_name.bold());
                #[cfg(feature = "colorization")]
                colorization_markers.push((variable_name, style::Clear + foreground::Red));
            }
            ASTBuildingError::CannotParseInteger { value, lower_bound, upper_bound } => {
                explanation = format!("Integer Value {} is not a number between {lower_bound} and {upper_bound}.", value.bold());
                #[cfg(feature = "colorization")]
                colorization_markers.push((value, style::Clear + foreground::Red));
            }
            ASTBuildingError::CannotParseDecimal { value, lower_bound, upper_bound } => {
                explanation = format!("Decimal Value {} is not a number between {lower_bound} and {upper_bound}.", value.bold());
                #[cfg(feature = "colorization")]
                colorization_markers.push((value, style::Clear + foreground::Red));
            }
        }

        let mut res = SimpleErrorExplanation::new()
            .explanation(explanation);
        if !solution.is_empty() {
            res = res.solution(solution);
        }
        #[cfg(feature = "colorization")]
        if !colorization_markers.is_empty() {
            res = res.colorization_markers(colorization_markers)
                .whole_input_colorization(foreground::true_color(150, 150, 150));
        }
        res
    }
}
