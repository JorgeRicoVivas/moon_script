extern crate pest_derive;


use std::time::{Duration, Instant};

use itertools::Itertools;
use pest::iterators::Pair;
use pest::RuleType;

use block_parsing::{Base, CompiletimeVariableInformation, ContextBuilder, FunctionDefinition};

use crate::block_parsing::{AST, Rule};
use crate::block_parsing::value_parsing::{FullValue, ReducedValue};
use crate::execution::optimized::OptimizedAST;

pub const FUNCTION_ELEMENTS_LEN: usize = 4;

pub mod external_utils;
pub mod execution;

static mut NUM_A: u128 = 15;
static mut NUM_B: u128 = 157;

fn main() {
    //simple_logger::init_with_level(log::Level::Trace);
    println!("Start");
    let mut base = Base::default();

    base.add_constant("MY_CONST", 5);
    /*
    base.add_function(FunctionDefinition::new("object_function", |_| Ok(ReducedValue::Boolean(true)))
        .knwon_return_type_name("boolean").associated_type_name("MyCustomType").module_name("Mod"));
    base.add_function(FunctionDefinition::new("function", |_| Ok(ReducedValue::Boolean(true)))
        .knwon_return_type_name("boolean").module_name("Mod"));
    base.add_function(FunctionDefinition::new("get_asocA", |_| unsafe { Ok(NUM_A.into()) })
        .knwon_return_type_name("int").associated_type_name("int").module_name("Mod"));
    base.add_function(FunctionDefinition::new("get_asocB", |_| unsafe { Ok(NUM_B.into()) })
        .knwon_return_type_name("int").associated_type_name("int").module_name("Mod"));
    base.add_function(FunctionDefinition::new("set_asocA",
                                              |mut values| unsafe {
                                                  if values.len() < 1 { return Err("Expected at least one parameter".to_string()); }
                                                  NUM_A = values.swap_remove(0).try_into().map_err(|_| "Error value was not a number".to_string())?;
                                                  Ok(().into())
                                              })
        .knwon_return_type_name("int").associated_type_name("int").module_name("Mod"));
    base.add_function(FunctionDefinition::new("set_asocB",
                                              |mut values| unsafe {
                                                  if values.len() < 1 { return Err("Expected at least one parameter".to_string()); }
                                                  NUM_B = values.swap_remove(0).try_into().map_err(|_| "Error value was not a number".to_string())?;
                                                  Ok(().into())
                                              })
        .knwon_return_type_name("int").associated_type_name("int").module_name("Mod"));

    base.add_function(FunctionDefinition::new("get_asocA", |_| unsafe { Ok(NUM_A.into()) })
        .knwon_return_type_name("int").associated_type_name("my_custom_type").module_name("Mod"));
    base.add_function(FunctionDefinition::new("set_asocB",
                                              |mut values| unsafe {
                                                  if values.len() < 1 { return Err("Expected at least one parameter".to_string()); }
                                                  NUM_B = values.swap_remove(0).try_into().map_err(|_| "Error value was not a number".to_string())?;
                                                  Ok(().into())
                                              })
        .knwon_return_type_name("int").associated_type_name("my_custom_type").module_name("Mod"));
    */

    base.add_function(FunctionDefinition::new("get_val", || 123890));
    let base = base;

    let mut context = ContextBuilder::default();
    context.push_variable(CompiletimeVariableInformation::new("first_var").value(5020));
    context.push_variable(CompiletimeVariableInformation::new("ident").value(2050).associated_type("MyCustomType"));
    context.push_variable(CompiletimeVariableInformation::new("d").associated_type("my_custom_type"));
    context.push_variable(CompiletimeVariableInformation::new("e").associated_type("my_custom_type"));
    context.push_variable(CompiletimeVariableInformation::new("f").associated_type("my_custom_type"));
    context.push_variable(CompiletimeVariableInformation::new("g").associated_type("int"));


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


    let res = base.parse(INPUT, context.clone()).unwrap();

    let optimized_ast = OptimizedAST::from(res.clone());
    let optimized_executor = optimized_ast.executor();

    let mut marks = Vec::new();
    const REPS: i32 = 20;
    for i in 0..REPS {
        let now = Instant::now();
        let _res = base.parse(INPUT, context.clone()).unwrap();
        let elapsed = now.elapsed();
        if i > 10 {
            marks.push(elapsed);
        }
    }
    let marks_len = marks.len();
    let duration_sum = marks.into_iter().sum::<Duration>();
    println!("-Parsing to AST avg speed: {:?}", duration_sum / marks_len as u32);


    test_speed(res.clone(), optimized_ast.clone());
    test_speed(res.clone(), optimized_ast.clone());
}

fn test_speed(res: AST, optimized_ast: OptimizedAST) {
    const MINIMUM_TO_COUNT: i32 = 10;
    const REPS: i32 = MINIMUM_TO_COUNT + (20000);
    let mut marks = Vec::new();
    for i in 0..REPS {
        let now = Instant::now();
        let _execution_res = res.executor()
            .push_variable("d", 1)
            .push_variable("e", 2)
            .push_variable("f", 3)
            .execute();
        let elapsed = now.elapsed();
        if i > MINIMUM_TO_COUNT {
            marks.push(elapsed);
        }
    }
    let marks_len = marks.len();
    let duration_sum = marks.into_iter().sum::<Duration>();
    println!("-Standard avg speed: {:?}", (duration_sum / marks_len as u32).as_secs_f64());
    println!("-sum: {:?}", duration_sum.as_secs_f64());

    let mut marks = Vec::new();
    for i in 0..REPS {
        let executor = res.executor()
            .push_variable("d", 1)
            .push_variable("e", 2)
            .push_variable("f", 3);
        let now = Instant::now();
        let _execution_res = executor.execute();
        let elapsed = now.elapsed();
        if i > MINIMUM_TO_COUNT {
            marks.push(elapsed);
        }
    }
    let marks_len = marks.len();
    let duration_sum = marks.into_iter().sum::<Duration>();
    println!("-Only exec avg speed: {:?}", (duration_sum / marks_len as u32).as_secs_f64());
    println!("-sum: {:?}", duration_sum.as_secs_f64());

    let mut marks = Vec::new();
    for i in 0..REPS {
        let now = Instant::now();
        let _execution_res = optimized_ast.executor()
            .push_variable("d", 1)
            .push_variable("e", 2)
            .push_variable("f", 3)
            .execute();
        let elapsed = now.elapsed();
        if i > MINIMUM_TO_COUNT {
            marks.push(elapsed);
        }
    }
    let marks_len = marks.len();
    let duration_sum = marks.into_iter().sum::<Duration>();
    println!("-Standard optimized avg speed: {:?}", (duration_sum / marks_len as u32).as_secs_f64());
    println!("-sum: {:?}", duration_sum.as_secs_f64());

    let mut marks = Vec::new();
    for i in 0..REPS {
        let executor = optimized_ast.executor()
            .push_variable("d", 1)
            .push_variable("e", 2)
            .push_variable("f", 0.3);
        let now = Instant::now();
        let _execution_res = executor.execute();
        let elapsed = now.elapsed();
        if i > MINIMUM_TO_COUNT {
            marks.push(elapsed);
        }
    }
    let marks_len = marks.len();
    let duration_sum = marks.into_iter().sum::<Duration>();
    println!("-Only exec optimized avg speed: {:?}", (duration_sum / marks_len as u32).as_secs_f64());
    println!("-sum: {:?}", duration_sum.as_secs_f64());
    let values = vec![FullValue::Boolean(true), FullValue::Integer(8)];
    test_function(Box::new(values.into_iter().map(|value| ReducedValue::try_from(value).unwrap())));

    let mut marks = Vec::new();
    for i in 0..REPS {
        let now = Instant::now();
        let _execution_res = optimized_ast.executor().execute();
        let elapsed = now.elapsed();
        if i > MINIMUM_TO_COUNT {
            marks.push(elapsed);
        }
    }
    let marks_len = marks.len();
    let duration_sum = marks.into_iter().sum::<Duration>();
    println!("-Standard optimized no variables avg speed: {:?}", (duration_sum / marks_len as u32).as_secs_f64());
    println!("-sum: {:?}", duration_sum.as_secs_f64());
}

fn test_function(a: Box<dyn Iterator<Item=ReducedValue>>) {
    a.for_each(|val| println!("{val}"));
}

mod reduced_value_impl;

pub mod block_parsing;
pub mod function;

/*
const INPUT: &'static str = r#"
            let a = 1;
            a = 3;
            b = a+2;
            c = a.asocA.asocB;
            a.asocA.asocB=1061;
            Mod/function(a, b);


            if true {
            } else{
                print("Negative branch that should never appear in AST")
            }

            if a.asocA {
                g = a.asocA.asocB;
                a = a.asocA.asocB;
            } else{
                h = a.asocA.asocB;
                println("Negative branch for a.asocA" a);
            }

            a = 10

            k = a.asocA.asocB;
            k = a.asocA.asocB;


            d.asocA.asocB=d.asocA.asocB;
            e.asocA.asocB=d.asocA.asocB;
            f.asocA.asocB=d.asocA.asocB;

            return f.asocA.asocB;
            "#;
 */

const INPUT: &'static str = r#"f = get_val();  if f<0{return 0;} else { if f>1{return 1;} else {return f;} }"#;

fn print_pairs_iter<'a, Rule: RuleType, RuleIter: Iterator<Item=Pair<'a, Rule>>>(pairs: RuleIter, ident: u8) {
    pairs.into_iter().for_each(|p| {
        let tabs = (0..ident).into_iter().map(|_| " - ").join("");
        println!("{tabs}{:?} = {}", p.as_rule(), p.as_str());
        print_pairs_iter(p.into_inner(), ident + 1);
    });
}