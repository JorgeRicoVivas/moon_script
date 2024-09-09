#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
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
type HashMap<K, V> = std::collections::HashMap<K, V>;

#[cfg(not(feature = "std"))]
type HashMap<K, V> = alloc::collections::BTreeMap<K, V>;

pub mod engine;
pub(crate) mod external_utils;
pub mod execution;
mod reduced_value_impl;
pub mod parsing;
pub mod function;
pub mod value;

#[cfg(test)]
mod test {
    use crate::engine::context::ContextBuilder;
    use crate::engine::Engine;
    use crate::{FunctionDefinition, InputVariable};
    use log::Level;

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

        engine.parse(r#"print("Should not be true: "+(!agent.is_flag() && agent.get_bool())); "#, context)
            .unwrap().executor().execute().expect("TODO: panic message");
    }

    #[test]
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

        engine.add_function(crate::parsing::FunctionDefinition::new("kill", |()| println!("Internal killing"))
            .associated_type_name("effect").known_return_type_name("effect"));
        engine.add_function(crate::parsing::FunctionDefinition::new("effect", |()| 1)
            .associated_type_name("effect").known_return_type_name("effect"));

        let ast = engine.parse(r#"
        agent.alt%2==1


        "#, context).map_err(|error| panic!("{error}"));
        println!("{ast:#?}");
        println!("{:#?}", ast.unwrap().executor().execute());
    }
}



