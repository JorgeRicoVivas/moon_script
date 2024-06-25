#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
pub extern crate pest;


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


/*

pub const FUNCTION_ELEMENTS_LEN: usize = 4;

static mut NUM_A: u128 = 15;
static mut NUM_B: u128 = 157;


#[test]
fn real_test() {
    main();
}

fn main() {
    simple_logger::init_with_level(log::Level::Trace);
    println!("Start");
    let mut base = Base::default();

    base.add_constant("MY_CONST", 5);


    base.add_function(FunctionDefinition::new("object_function", || VBValue::Boolean(true))
        .knwon_return_type_name("boolean").associated_type_name("MyCustomType").module_name("Mod"));
    base.add_function(FunctionDefinition::new("function", || VBValue::Boolean(true))
        .knwon_return_type_name("boolean").module_name("Mod"));
    base.add_function(FunctionDefinition::new("get_asocA", || unsafe { NUM_A })
        .knwon_return_type_name("int").associated_type_name("int").module_name("Mod"));
    base.add_function(FunctionDefinition::new("get_asocB", || unsafe { NUM_B })
        .knwon_return_type_name("int").associated_type_name("int").module_name("Mod"));
    base.add_function(FunctionDefinition::new("set_asocA",
                                              |mut values: Vec<u8>| -> Result<(), String> {
                                                  unsafe {
                                                      if values.len() < 1 { return Err("Expected at least one parameter".to_string()); }
                                                      NUM_A = values.swap_remove(0).try_into().map_err(|_| "Error value was not a number".to_string())?;
                                                      Ok(().into())
                                                  }
                                              })
        .knwon_return_type_name("int").associated_type_name("int").module_name("Mod"));
    base.add_function(FunctionDefinition::new("set_asocB",
                                              |mut values: Vec<u8>| -> Result<(), String> {
                                                  unsafe {
                                                      if values.len() < 1 { return Err("Expected at least one parameter".to_string()); }
                                                      NUM_B = values.swap_remove(0).try_into().map_err(|_| "Error value was not a number".to_string())?;
                                                      Ok(().into())
                                                  }
                                              })
        .knwon_return_type_name("int").associated_type_name("int").module_name("Mod"));

    base.add_function(FunctionDefinition::new("get_asocA", || unsafe { NUM_A })
        .knwon_return_type_name("int").associated_type_name("my_custom_type").module_name("Mod"));
    base.add_function(FunctionDefinition::new("set_asocB",
                                              |mut values: Vec<u8>| -> Result<(), String> {
                                                  unsafe {
                                                      if values.len() < 1 { return Err("Expected at least one parameter".to_string()); }
                                                      NUM_B = values.swap_remove(0).try_into().map_err(|_| "Error value was not a number".to_string())?;
                                                      Ok(().into())
                                                  }
                                              })
        .knwon_return_type_name("int").associated_type_name("my_custom_type").module_name("Mod"));


    base.add_function(FunctionDefinition::new("get_val", || 123890));
    base.add_function(FunctionDefinition::new("get_asocA", || 12).associated_type_name("my_custom_type"));

    let base = base;

    let mut context = ContextBuilder::default();
    /*
    context.push_variable(CompiletimeVariableInformation::new("first_var").value(5020));
    context.push_variable(CompiletimeVariableInformation::new("ident").value(2050).associated_type("MyCustomType"));
    context.push_variable(CompiletimeVariableInformation::new("d").associated_type("my_custom_type"));
    context.push_variable(CompiletimeVariableInformation::new("e").associated_type("my_custom_type"));
    context.push_variable(CompiletimeVariableInformation::new("f").associated_type("my_custom_type"));
    context.push_variable(CompiletimeVariableInformation::new("g").associated_type("int"));

    */

    context.push_variable(CompiletimeVariableInformation::new("lazy").associated_type("my_custom_type").lazy_value(|| {
        14
    }));

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

    let parsed = base.parse(INPUT, context
        .clone()
        .with_start_parsing_position_offset(3, 10)
        .with_parsing_column_fixed(true),
    );
    if parsed.is_err() {
        panic!("Err:\n{}", parsed.err().unwrap());
    }
    let res = parsed.unwrap();
    println!("{res:#?}");

    let optimized_ast = res.clone().to_optimized_ast();
    let optimized_executor = optimized_ast.executor();

    println!("{:?}", optimized_executor.execute());

    test_speed(res.clone(), optimized_ast.clone());
    test_speed(res.clone(), optimized_ast.clone());
}

fn test_speed(res: AST, optimized_ast: OptimizedAST) {
    const MINIMUM_TO_COUNT: i32 = 10;
    const REPS: i32 = MINIMUM_TO_COUNT + (20000);
    let mut marks = Vec::with_capacity((REPS - MINIMUM_TO_COUNT) as usize);
    for i in 0..REPS {
        let now = Instant::now();
        let _execution_res = res.executor()
            .push_variable("d", 1)
            .push_variable("e", 2)
            .push_variable("f", 3)
            .push_variable("lazy", 3)
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

    let mut marks = Vec::with_capacity((REPS - MINIMUM_TO_COUNT) as usize);
    for i in 0..REPS {
        let executor = res.executor()
            .push_variable("d", 1)
            .push_variable("e", 2)
            .push_variable("f", 3)
            .push_variable("lazy", 3);
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

    let mut marks = Vec::with_capacity((REPS - MINIMUM_TO_COUNT) as usize);
    for i in 0..REPS {
        let now = Instant::now();
        let _execution_res = optimized_ast.executor()
            .push_variable("d", 1)
            .push_variable("e", 2)
            .push_variable("f", 3)
            .push_variable("lazy", 3)
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

    let mut marks = Vec::with_capacity((REPS - MINIMUM_TO_COUNT) as usize);
    for i in 0..REPS {
        let executor = optimized_ast.executor()
            .push_variable("d", 1)
            .push_variable("e", 2)
            .push_variable("f", 0.3)
            .push_variable("lazy", 3);
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


    let mut marks = Vec::with_capacity((REPS - MINIMUM_TO_COUNT) as usize);
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


    let optimized_ast = Box::new(optimized_ast);
    let mut marks = Vec::with_capacity((REPS - MINIMUM_TO_COUNT) as usize);
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
    println!("-Standard optimized no variables but Box avg speed: {:?}", (duration_sum / marks_len as u32).as_secs_f64());
    println!("-Standard optimized no variables but Box avg speed: {:?}", (duration_sum / marks_len as u32));
    println!("-sum: {:?}", duration_sum.as_secs_f64());

    let optimized_ast = Box::new(optimized_ast);
    let mut marks = Vec::with_capacity((REPS - MINIMUM_TO_COUNT) as usize);
    for i in 0..REPS {
        let now = Instant::now();
        let _execution_res = optimized_ast.executor().execute_stack();
        let elapsed = now.elapsed();
        if i > MINIMUM_TO_COUNT {
            marks.push(elapsed);
        }
    }
    let marks_len = marks.len();
    let duration_sum = marks.into_iter().sum::<Duration>();
    println!("-Standard optimized no variables but Box and stack avg speed: {:?}", (duration_sum / marks_len as u32).as_secs_f64());
    println!("-Standard optimized no variables but Box and stack avg speed: {:?}", (duration_sum / marks_len as u32));
    println!("-sum: {:?}", duration_sum.as_secs_f64());


    let sum = 0;
    let mut marks = Vec::with_capacity((REPS - MINIMUM_TO_COUNT) as usize);
    let alloc_start = Instant::now();
    for i in 0..REPS {
        let now = Instant::now();
        let elapsed = now.elapsed();
        if i > MINIMUM_TO_COUNT {
            marks.push(elapsed);
        }
    }
    let alloc_end = alloc_start.elapsed();
    let marks_len = marks.len();
    let duration_sum = marks.into_iter().sum::<Duration>();
    println!("-Static op avg speed: {:?}", (duration_sum / marks_len as u32).as_secs_f64());
    println!("-Static op avg speed: {:?}", (duration_sum / marks_len as u32));
    println!("-sum: {:?}", duration_sum.as_secs_f64());
    println!("-bench: {:?}", alloc_end.as_secs_f64());
    unsafe { println!("-Static: {SUM}"); }
}

static mut SUM: i128 = 0;

fn test_function(a: Box<dyn Iterator<Item=VBValue>>) {
    a.for_each(|val| println!("{val}"));
}


const INPUT: &'static str = r#"nonexisting_function(aasdjk,asd);
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


            Mod/function(aasdjk,asd);
            d.asocA.asocB=d.asocA.asocB;
            e.asocA.asocB=d.asocA.asocB;
            f.asocA.asocB=d.asocA.asocB;

            if a.asocA.asocB {
                e.asocA.asocB=d.asocA.asocB;
            } else{
                e.asocA.asocB=d.asocA.asocB;
            }

            adjksn(aasdjk,asd);
            return f.asocA.asocB;
            "#;


// INPUT: &'static str = r#"let lazy = get_val(); if lazy<0{return 0;} else if lazy>20{return 20;} else {return lazy;}"#;

/*
const INPUT: &'static str = r#"
    has_no_let = 5;
    let has_let = 5;

    let a = 10
    while a > 0{
        print("Current value of a is "+a);
        a = a - 1;
    }
    print("Last value of a is "+a);

    if lazy.asocA>20 {
        return 20;
    } else if lazy.asocA<0 {
        return 0;
    } else {
        return lazy.asocA;
    }
    "#;


 */

/*
const INPUT: &'static str = r#"
    a = 10
    print("First value of a is "+a)
    b = 1
    while b<5{
        if b<2 {
            b = 5
            a = 2
            print("After change value of a is "+a)
        }
    }
    "#;
*/

fn print_pairs_iter<'a, Rule: RuleType, RuleIter: Iterator<Item=Pair<'a, Rule>>>(pairs: RuleIter, ident: u8) {
    pairs.into_iter().for_each(|p| {
        let tabs = (0..ident).into_iter().map(|_| " - ").join("");
        println!("{tabs}{:?} = {}", p.as_rule(), p.as_str());
        print_pairs_iter(p.into_inner(), ident + 1);
    });
}
*/