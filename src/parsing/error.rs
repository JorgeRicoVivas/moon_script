use alloc::fmt::{Debug, Display, Formatter};
use alloc::format;
use alloc::string::String;

#[cfg(feature = "colorization")]
use colored::Colorize;
use pest::error::LineColLocation;
use simple_detailed_error::{SimpleError, SimpleErrorDetail, SimpleErrorExplanation};
#[cfg(feature = "colorization")]
use string_colorization::{foreground, style};

use crate::execution::RuntimeError;
use crate::parsing::Rule;

#[derive(Debug)]
pub enum ParsingError<'input> {
    Parsing(pest::error::Error<Rule>),
    CouldntBuildAST(SimpleError<'input>),
}

impl<'input> Display for ParsingError<'input> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            ParsingError::Parsing(pest_error) => f.write_str(&format!("{pest_error}")),
            ParsingError::CouldntBuildAST(simple_error) => f.write_str(&format!("{}", simple_error.as_display_struct(true))),
        }
    }
}

impl<'input> From<ParsingError<'input>> for SimpleError<'input> {
    fn from(value: ParsingError<'input>) -> Self {
        match value {
            ParsingError::Parsing(parsing) => {
                let mut error = SimpleError::new()
                    .error_detail(format!("On {} because of {}", parsing.line(), parsing.variant));
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

#[derive(Debug)]
pub enum ASTBuildingError<'input> {
    VariableNotInScope { variable_name: &'input str },
    OperatorNotFound { operator: &'input str },
    FunctionNotFound { function_name: &'input str, associated_to_type: Option<String>, module: Option<&'input str> },
    PropertyFunctionNotFound { preferred_property_to_find: String, original_property: &'input str, typename: String },
    CouldntInlineFunction { function_name: &'input str, runtime_error: RuntimeError },
    CouldntInlineGetter { execution_error_message: String, property: &'input str },
    CouldntInlineUnaryOperator { operator: &'input str, runtime_error: RuntimeError },
    CouldntInlineBinaryOperator { operator: &'input str, runtime_error: RuntimeError },
    CouldntInlineVariableOfUnknownType { variable_name: &'input str },
    CannotParseInteger { value: &'input str, lower_bound: i128, upper_bound: i128 },
    CannotParseDecimal { value: &'input str, lower_bound: f64, upper_bound: f64 },
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
