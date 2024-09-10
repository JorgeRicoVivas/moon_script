//! [![crates.io](https://img.shields.io/crates/v/moon_script.svg)](https://crates.io/crates/moon_script)
//! [![docs.rs](https://img.shields.io/docsrs/moon_script)](https://docs.rs/moon_script/latest/moon_script/)
//! [![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/JorgeRicoVivas/moon_script/rust.yml)](https://github.com/JorgeRicoVivas/moon_script/actions)
//! [![GitHub last commit](https://img.shields.io/github/last-commit/JorgeRicoVivas/moon_script)](https://github.com/JorgeRicoVivas/moon_script)
//! [![GitHub License](https://img.shields.io/github/license/JorgeRicoVivas/moon_script)](https://github.com/JorgeRicoVivas/moon_script?tab=CC0-1.0-1-ov-file)
//!
//! MoonScript is a very basic scripting language for simple scripting with some syntax based on
//! Rust's, the idea of MoonScript it's for those writing MoonScript to find themselves scripts in
//! the simplest manner possible while still boosting performance.
//!
//! If you want a tour on MoonScript, feel free to check the
//! [web book](https://jorgericovivas.github.io/moon_script_book/) out!
//!
//! ## Features
//! - std (Default): MoonScript will target the Standard library, implementing the Error trait on
//! error types and using Sync with std::sync mechanisms where possible.
//! - colorization (Default): Parsing errors will get colorized when printing them in the terminal.
//! - medium_functions: Functions added to an Engine can be up to 16 parameters, instead of 8.
//! - big_functions: Functions added to an Engine can be up to 24 parameters, instead of 8.
//! - massive_functions: Functions added to an Engine can be up to 40 parameters, instead of 8.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
extern crate core;
extern crate pest;

pub use engine::context::ContextBuilder;
pub use engine::context::InputVariable;
pub use engine::Constant;
pub use engine::Engine;

pub use execution::ast::ASTExecutor;
pub use execution::ast::AST;
pub use execution::RuntimeError;

pub use execution::optimized_ast::OptimizedAST;
pub use execution::optimized_ast::OptimizedASTExecutor;

pub use function::ToAbstractFunction;

pub use parsing::error::ASTBuildingError;
pub use parsing::error::ParsingError;
pub use parsing::FunctionDefinition;
pub use parsing::MoonValueKind;

pub use value::MoonValue;


#[cfg(feature = "std")]
type HashSet<T> = std::collections::HashSet<T>;
#[cfg(feature = "std")]
type HashMap<K, V> = std::collections::HashMap<K, V>;
#[cfg(feature = "std")]
type LazyLock<T> = std::sync::LazyLock<T>;


#[cfg(not(feature = "std"))]
type HashMap<K, V> = alloc::collections::BTreeMap<K, V>;
#[cfg(not(feature = "std"))]
type LazyLock<T> = lazy_lock::LazyLock<T>;
#[cfg(not(feature = "std"))]
type HashSet<T> = alloc::collections::btree_set::BTreeSet<T>;


pub mod engine;
pub(crate) mod external_utils;
pub mod execution;
mod reduced_value_impl;
pub mod parsing;
pub mod function;
pub mod value;

#[cfg(not(feature = "std"))]
pub(crate) mod lazy_lock;


#[cfg(test)]
mod test {
    use crate::engine::context::ContextBuilder;
    use crate::engine::Engine;
    use crate::{FunctionDefinition, InputVariable};
    use log::Level;

    #[cfg(feature = "std")]
    #[test]
    fn test_optimizations() {
        let mut engine = Engine::new();
        engine.add_constant("ONE_AS_CONSTANT", 1);
        engine.add_function(FunctionDefinition::new("constant_fn_get_two", || { 2 }).inline());
        let context_with_a_constant_input_variable = ContextBuilder::new()
            .with_variable(InputVariable::new("four").value(4));

        let unoptimized_script_source = r###"
            let three = ONE_AS_CONSTANT + constant_fn_get_two();
            if three == 3{
                print("First line!");
            } else if three!=3 {
                print("How?");
            } else {
                print("This won't ever happen 1");
            }
            if three!=3 {
                print("This wont ever happen either 1");
            } else {
                print("Second line!");
            }
            if four == 4 {
                print("Third line!");
            } else if four!=4 {
                print("How?");
            } else {
                print("This won't ever happen 2");
            }
            if four!=4 {
                print("This wont ever happen either 2");
            } else {
                print("Fourth line!");
            }
            while three == 3 {
                print("Eternal loop!");
            }
        "###;

        let optimized_script_source = r###"
            print("First line!");
            print("Second line!");
            print("Third line!");
            print("Fourth line!");
            while true {
                print("Eternal loop!");
            }
        "###;

        let ast_from_optimized = Engine::new()
            .parse(optimized_script_source, context_with_a_constant_input_variable.clone()).unwrap();
        let ast_from_unoptimized = engine
            .parse(unoptimized_script_source, context_with_a_constant_input_variable.clone()).unwrap();

        assert_eq!(ast_from_optimized, ast_from_unoptimized);
    }

    #[test]
    fn test_array() {
        simple_logger::init_with_level(Level::Trace);
        let engine = Engine::default();

        let ast = engine.parse("let a = [[4 2 5] [3 9 1] [6 8 7]]; a[1][2]", Default::default())
            .unwrap();
        let moon_result: i32 = ast.executor().execute().unwrap().try_into().unwrap();

        let rust_executed = (|| {
            let a = [[4, 2, 5], [3, 9, 1], [6, 8, 7]];
            a[1][2]
        })();
        assert_eq!(rust_executed, moon_result);
    }

    #[test]
    fn test_precedence() {
        simple_logger::init_with_level(Level::Trace);
        let engine = Engine::default();
        let expected = 2 * 3 + 5 > 4 && true;
        let moon_result: bool = engine.parse("2 * 3 + 5 > 4 && true", Default::default())
            .unwrap().executor().execute().unwrap().try_into().unwrap();
        assert_eq!(expected, moon_result);

        let expected = true && 4 < 5 + 3 * 2;
        let moon_result: bool = engine.parse("true && 4 < 5 + 3 * 2", Default::default())
            .unwrap().executor().execute().unwrap().try_into().unwrap();
        assert_eq!(expected, moon_result);
    }

    #[test]
    fn test_binary_comparator_and_unary() {
        simple_logger::init_with_level(Level::Trace);
        let mut engine = Engine::default();
        engine.add_function(FunctionDefinition::new("is_flag", |()| false)
            .associated_type_name("agent").known_return_type_name("bool"));
        engine.add_function(crate::parsing::FunctionDefinition::new("get_bool", |()| false)
            .associated_type_name("agent").known_return_type_name("bool"));

        let mut context = ContextBuilder::default();
        context.push_variable(crate::engine::context::InputVariable::new("agent")
            .associated_type("agent")
            .lazy_value(|| 46397));

        let res : bool = engine.parse(r#"(!agent.is_flag() && agent.get_bool())"#, context)
            .unwrap().executor().execute().expect("TODO: panic message").try_into().unwrap();
        assert_eq!(false, res);
    }

    #[cfg_attr(not(feature = "std"), test)]
    fn test_custom_unnamed_type() {
        let _ = simple_logger::init_with_level(log::Level::Trace);

        let mut engine = Engine::default();
        let mut context = ContextBuilder::default();
        context.push_variable(crate::engine::context::InputVariable::new("agent")
            .lazy_value(|| 46397)
            .associated_type("agent"));
        context.push_variable(crate::engine::context::InputVariable::new("effect")
            .lazy_value(|| 377397)
            .associated_type("effect"));

        context.push_variable(crate::engine::context::InputVariable::new("forced_true")
            .associated_type("boolean")
            .lazy_value(|| true));

        engine.add_function(crate::parsing::FunctionDefinition::new("alt", |()| 0)
            .associated_type_name("agent").known_return_type_name("int"));
        engine.add_function(crate::parsing::FunctionDefinition::new("is_flag", |()| false)
            .associated_type_name("agent").known_return_type_name("bool"));


        engine.add_function(crate::parsing::FunctionDefinition::new("set_scale",
                                                                    |(), _scale: f32| {}, )
            .associated_type_name("effect"));
        engine.add_function(crate::parsing::FunctionDefinition::new("lived_time",
                                                                    |()| { 3 }, )
            .associated_type_name("effect"));
        engine.add_function(crate::parsing::FunctionDefinition::new("set_pos",
                                                                    |(), _x: f32, _y: f32, _z: f32| {},
        ).associated_type_name("effect"));
        engine.add_function(crate::parsing::FunctionDefinition::new("set_color",
                                                                    |(), x: f32, y: f32, z: f32| {
                                                                        x + y + z
                                                                    },
        ).associated_type_name("effect"));

        engine.add_function(crate::parsing::FunctionDefinition::new("kill", |()| {
            #[cfg(feature = "std")]
            println!("Removing effect");
        })
            .associated_type_name("effect").known_return_type_name("effect"));
        engine.add_function(crate::parsing::FunctionDefinition::new("effect", |()| 1)
            .associated_type_name("effect").known_return_type_name("effect"));

        let ast = engine.parse(r#"
        agent.alt%2==1


        "#, context).map_err(|error| panic!("{error}"));
        ast.unwrap().executor().execute().unwrap();
    }
}

#[cfg(test)]
mod book_tests {
    use crate::{ContextBuilder, Engine, FunctionDefinition, InputVariable, MoonValue};
    use alloc::format;
    use alloc::string::{String, ToString};


    #[cfg(feature = "std")]
    #[test]
    fn developers_guide___engine() {
        let engine = Engine::new();
        let context = ContextBuilder::new();

        // Create an AST out of a script that prints to the standard output
        let ast = engine.parse(r###"println("Hello world")"###, context).unwrap();

        /// Execute the AST
        ast.execute();
    }

    #[test]
    fn developers_guide___engine___add_constants() {
        let mut engine = Engine::new();

        // Create a constant named ONE
        engine.add_constant("ONE", 1);

        // Creates and executes a script that returns the constant
        let ast_result = engine.parse(r###"return ONE;"###, ContextBuilder::new()).unwrap()
            .execute().unwrap();

        assert_eq!(MoonValue::Integer(1), ast_result);

        // The value returned by an AST execution is a MoonValue, luckily, MoonValue implements
        // TryFrom for basic rust primitives and String, so we can get it with try_into()
        // as an i32
        let ast_result_as_i32: i32 = ast_result.try_into().unwrap();
        assert_eq!(1, ast_result_as_i32);
    }

    #[test]
    fn developers_guide___engine___add_functions() {
        let mut engine = Engine::new();

        // Creates a function that adds two numbers, this function is a function that can be
        // called at compile time, so we also call 'inline' to enable this optimization.
        let function_sum_two = FunctionDefinition::new("sum_two", |n: u8, m: u8| n + m)
            .inline();

        // The function is added to the engine
        engine.add_function(function_sum_two);

        // Creates and executes a script that sums 1 and 2 and returns its result
        let ast_result: i32 = engine.parse(r###"sum_two(1,2);"###, ContextBuilder::new())
            .unwrap().execute().unwrap().try_into().unwrap();
        assert_eq!(3, ast_result);
    }

    #[test]
    fn developers_guide___engine___add_functions__Result() {
        let mut engine = Engine::new();

        // Creates a function that adds two numbers, this function is a function that can be
        // called at compile time, so we also call 'inline' to enable this optimization.
        let function_sum_two = FunctionDefinition::new("sum_two", |n: u8, m: u8| n.checked_add(m).ok_or(format!("Error, numbers too large ({n}, {m})")))
            .inline();

        // The function is added to the engine
        engine.add_function(function_sum_two);

        // Creates and executes a script that sums 1 and 2 and returns its result, not failing
        let ast_result: i32 = engine.parse(r###"sum_two(1,2);"###, ContextBuilder::new())
            .unwrap().execute().unwrap().try_into().unwrap();
        assert_eq!(3, ast_result);

        // Creates and executes a script that sums 100 and 200, forcing the compilation to fail
        let error = engine.parse(r###"sum_two(100,200);"###, ContextBuilder::new())
            .err().unwrap().couldnt_build_ast_error().unwrap().as_display_struct(false);
        let compilation_error = format!("{}", error);
        #[cfg(feature = "std")]
        println!("{}", compilation_error);

        assert_eq!(compilation_error.replace(" ", "").replace("\n", ""), (r###"
            Error: Could not compile.
            Cause:
              - Position: On line 1 and column 1
              - At: sum_two(100,200)
              - Error: The constant function sum_two was tried to be inlined, but it returned this error:
                       Could not execute a function due to: Error, numbers too large (100, 200).
                       "###).replace(" ", "").replace("\n", ""));
    }

    #[derive(Clone)]
    struct MyType {
        name: String,
        age: u16,
    }

    impl From<MyType> for MoonValue {
        fn from(value: MyType) -> Self {
            MoonValue::from([
                MoonValue::from(value.name),
                MoonValue::from(value.age)
            ])
        }
    }

    impl TryFrom<MoonValue> for MyType {
        type Error = ();

        fn try_from(value: MoonValue) -> Result<Self, Self::Error> {
            match value {
                MoonValue::Array(mut moon_values) => Ok(
                    Self {
                        name: String::try_from(moon_values.remove(0)).map_err(|_| ())?,
                        age: u16::try_from(moon_values.remove(0)).map_err(|_| ())?,
                    }
                ),
                _ => Err(())
            }
        }
    }

    #[test]
    fn developers_guide___engine___custom_type() {
        let mut engine = Engine::new();

        // Create a value for the type
        let my_type_example = MyType { name: "Jorge".to_string(), age: 23 };

        // Create a constant with said type
        engine.add_constant("BASE_HUMAN", my_type_example.clone());

        // Create a getter for the field age
        engine.add_function(FunctionDefinition::new("age", |value: MyType| {
            value.age
        })
            // The function is associated to the type 'MyType', this means the function age
            // will work as a property, so instead of calling 'age(BASE_HUMAN)',
            // we write 'BASE_HUMAN.age', for more information about properties, check the
            // properties secion of the user's guide
            .associated_type_of::<MyType>());

        // Create and execute a script that uses the custom constant and function, where
        // it gets the age of the human
        let age: u16 = engine.parse("BASE_HUMAN.age", Default::default())
            .unwrap().execute().unwrap().try_into().unwrap();

        assert_eq!(my_type_example.age, age);
    }

    #[test]
    fn developers_guide___context___input_variables() {
        let engine = Engine::new();

        // Create a context with an inlined variable named user_name whose value is 'Jorge'
        let context = ContextBuilder::new()
            .with_variable(InputVariable::new("user_name").value("Jorge"));

        // Creates and executes a script that returns the value of said variable
        let user_name: String = engine.parse("return user_name;", context)
            .unwrap().execute().unwrap().try_into().unwrap();

        assert_eq!("Jorge", user_name);
    }

    #[test]
    fn developers_guide___context___ast_input_variables() {
        let engine = Engine::new();

        // Create a context with a late variable named user_name whose type is that of a String
        let context = ContextBuilder::new()
            .with_variable(InputVariable::new("user_name").associated_type_of::<String>());

        // Compiles a script that returns the value of said variable and creates an executor to
        // execute said script, to give the value of this variable to the executor, the method
        // push_variable is called with the name of the late variable and it's value
        let ast = engine.parse("return user_name;", context)
            .unwrap();
        let ast_executor = ast.executor()
            .push_variable("user_name", "Jorge");

        // Executes the AST to return the value of said variable
        let user_name: String = ast_executor
            .execute().unwrap().try_into().unwrap();

        assert_eq!("Jorge", user_name);
    }

    #[test]
    fn developers_guide___context___line_error() {
        let engine = Engine::new();
        // The script we are going to make to fail on compilation has an error in line 5,
        // where it calls a function that does not exist
        let script = r###"
let a = 5;
let b = 10;
let sum = a + b;
calling_an_non_existing_function(sum)
        "###;

        // The default context builder didn't specify a starting position for the script.
        let context_builder = ContextBuilder::new();

        // The following line just parses the error as a string and searches for the line
        // specifying the position, don't worry; you will likely never do this.
        let error = engine.parse(script, context_builder).err().unwrap();

        #[cfg(feature = "std")]
        // Shows the error to the user, so he can look up what was wrong.
        println!("{error}");

        let simple_error = format!("{error}").lines().skip(2).next().unwrap().to_string();

        // The error is successfully located at line 5
        assert_eq!("  - Position: On line 5 and column 1", simple_error);

        // Now we will run the same test again, this time however, we will specify the script
        // has an offset of starting in line 100 and column 100 of a file.
        let context_builder = ContextBuilder::new()
            .with_start_parsing_position_offset(100, 100);
        let error = engine.parse(script, context_builder).err().unwrap();

        let simple_error = format!("{error}").lines().skip(2).next().unwrap().to_string();

        // Since the error happens on line 5 and column 1, this error happens on line 105 and
        // column 1 of the file.
        //
        // The reason for the column to be 1 and not 101 it's because it interprets each new line
        // starts at column 1, so it would have shown 101 if it was on line 1, but not in any
        // following, if the column is fixed, you can call
        // [ContextBuilder::parsing_column_fixed(true)], that way it would have shown column 101
        assert_eq!("  - Position: On line 105 and column 1", simple_error);
    }
}

