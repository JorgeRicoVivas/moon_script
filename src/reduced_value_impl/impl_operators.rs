use crate::block_parsing::{Base, FunctionInfo};
use crate::block_parsing::value_parsing::ReducedValue;

const ARITHMETIC_RESULT_BOOL: u8 = 0;
const ARITHMETIC_RESULT_INT: u8 = 1;
const ARITHMETIC_RESULT_DECIMAL: u8 = 2;

fn arithmetic_choice(arg1: ReducedValue, arg2: ReducedValue, on_bools: fn(bool, bool) -> Result<ReducedValue, String>, on_int: fn(i128, i128) -> Result<ReducedValue, String>, on_decimal: fn(f64, f64) -> Result<ReducedValue, String>) -> Result<Result<ReducedValue, String>, (ReducedValue, ReducedValue)> {
    match arithmetic_result(&arg1, &arg2) {
        None => Err((arg1, arg2)),
        Some(int) => {
            match int {
                ARITHMETIC_RESULT_BOOL => Ok(on_bools(TryInto::<bool>::try_into(arg1).unwrap(), TryInto::<bool>::try_into(arg2).unwrap())),
                ARITHMETIC_RESULT_INT => Ok(on_int(TryInto::<i128>::try_into(arg1).unwrap(), TryInto::<i128>::try_into(arg2).unwrap())),
                ARITHMETIC_RESULT_DECIMAL => Ok(on_decimal(TryInto::<f64>::try_into(arg1).unwrap(), TryInto::<f64>::try_into(arg2).unwrap())),
                _ => Err((arg1, arg2))
            }
        }
    }
}

fn arithmetic_result(arg1: &ReducedValue, arg2: &ReducedValue) -> Option<u8> {
    let top_left_level = match arg1 {
        ReducedValue::Boolean(_) => ARITHMETIC_RESULT_BOOL,
        ReducedValue::Integer(_) => ARITHMETIC_RESULT_INT,
        ReducedValue::Decimal(_) => ARITHMETIC_RESULT_DECIMAL,
        _ => return None,
    };
    let top_right_level = match arg2 {
        ReducedValue::Boolean(_) => ARITHMETIC_RESULT_BOOL,
        ReducedValue::Integer(_) => ARITHMETIC_RESULT_INT,
        ReducedValue::Decimal(_) => ARITHMETIC_RESULT_DECIMAL,
        _ => return None,
    };
    Some(if top_right_level >= top_left_level { top_right_level } else { top_left_level })
}

pub(crate) fn get_unary_operators() -> Vec<(&'static str, fn(ReducedValue) -> Result<ReducedValue, String>)> {
    vec![
        ("!", |arg| {
            match arg {
                ReducedValue::Boolean(bool) => Ok(ReducedValue::Boolean(!bool)),
                ReducedValue::Integer(int) => Ok(ReducedValue::Integer(!int)),
                ReducedValue::Null | ReducedValue::Decimal(_) | ReducedValue::String(_) | ReducedValue::Array(_) =>
                    Err("Unary operator '!' only can be applied between booleans or integers".to_string()),
            }
        }),
        ("-", |arg| {
            match arg {
                ReducedValue::Integer(int) => Ok(ReducedValue::Integer(-int)),
                ReducedValue::Decimal(dec) => Ok(ReducedValue::Decimal(-dec)),
                ReducedValue::Null | ReducedValue::Boolean(_) | ReducedValue::String(_) | ReducedValue::Array(_) =>
                    Err("Unary operator '-' only can be applied between integers or decimals".to_string()),
            }
        }),
    ]
}


pub(crate) fn get_binary_operators() -> Vec<(&'static str, fn(ReducedValue, ReducedValue) -> Result<ReducedValue, String>)> {
    vec![
        ("+", |arg_1, arg_2| {
            match (&arg_1, &arg_2) {
                (ReducedValue::String(string_1), ReducedValue::String(string_2)) => {
                    return Ok(ReducedValue::String(format!("{string_1}{string_2}")));
                }
                (ReducedValue::String(string_1), arg_2) => {
                    return Ok(ReducedValue::String(format!("{string_1}{arg_2}")));
                }
                (arg_1, ReducedValue::String(string_2)) => {
                    return Ok(ReducedValue::String(format!("{arg_1}{string_2}")));
                }
                _ => {}
            }

            match arithmetic_choice(arg_1, arg_2,
                                    |bool_1, bool_2| Ok(ReducedValue::Boolean(bool_1 || bool_2)),
                                    |int_1, int_2| Ok(ReducedValue::Integer(int_1.checked_add(int_2).unwrap_or(i128::MAX))),
                                    |dec_1, dec_2| Ok(ReducedValue::Decimal(dec_1 + dec_2))) {
                Ok(res) => { return res; }
                Err((arg_1, arg_2)) => {
                    Ok(match (arg_1, arg_2) {
                        (ReducedValue::Array(mut array_1), ReducedValue::Array(array_2)) => {
                            array_1.extend(array_2.into_iter());
                            ReducedValue::Array(array_1)
                        }
                        _ => return Err("Operator '+' can only be applied between booleans, integers, decimals, arrays or strings".to_string()),
                    })
                }
            }
        }),
        ("-", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(ReducedValue::Boolean(bool_1 && !bool_2)),
                              |int_1, int_2| Ok(ReducedValue::Integer(int_1.checked_sub(int_2).unwrap_or(i128::MIN))),
                              |dec_1, dec_2| Ok(ReducedValue::Decimal(dec_1 - dec_2)))
                .map_err(|_| "Operator '-' can only be applied between booleans, integers or decimals".to_string())?
        }),
        ("*", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(ReducedValue::Boolean(bool_1 && bool_2)),
                              |int_1, int_2| Ok(ReducedValue::Integer(int_1.checked_mul(int_2).unwrap_or(i128::MAX))),
                              |dec_1, dec_2| Ok(ReducedValue::Decimal(dec_1 * dec_2)))
                .map_err(|_| "Operator '*' can only be applied between booleans, integers or decimals".to_string())?
        }),
        ("/", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |_, _| Err("Operator '/' cannot be applied between booleans".to_string()),
                              |int_1, int_2| {
                                  let res = (int_1 as f64) / (int_2 as f64);
                                  if !res.is_normal() {
                                      Ok(ReducedValue::Integer(int_1.checked_div(int_2).unwrap_or(i128::MAX)))
                                  } else if res == (res as i128 as f64) {
                                      Ok(ReducedValue::Integer(res as i128))
                                  } else {
                                      Ok(ReducedValue::Decimal(res))
                                  }
                              },
                              |dec_1, dec_2| Ok(ReducedValue::Decimal(dec_1 / dec_2)))
                .map_err(|_| "Operator '/' can only be applied between integers or decimals".to_string())?
        }),
        ("%", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |_, _| Err("Operator '%' cannot be applied between booleans".to_string()),
                              |int_1, int_2| Ok(ReducedValue::Integer(int_1.checked_rem(int_2).unwrap_or(0))),
                              |dec_1, dec_2| Ok(ReducedValue::Decimal(dec_1 % dec_2)))
                .map_err(|_| "Operator '%' can only be applied between integers or decimals".to_string())?
        }),
        ("&&", |arg_1, arg_2| {
            Ok(match (arg_1, arg_2) {
                (ReducedValue::Boolean(bool_1), ReducedValue::Boolean(bool_2)) => {
                    ReducedValue::Boolean(bool_1 && bool_2)
                }
                args @ (ReducedValue::Decimal(_) | ReducedValue::Integer(_), ReducedValue::Decimal(_) | ReducedValue::Integer(_)) => {
                    let int_1 = TryInto::<i128>::try_into(args.0).unwrap();
                    let int_2 = TryInto::<i128>::try_into(args.1).unwrap();
                    ReducedValue::Integer(int_1 & int_2)
                }
                _ => return Err("Operator '&&' can only be applied between boolean, integers or decimals".to_string()),
            })
        }),
        ("^", |arg_1, arg_2| {
            Ok(match (arg_1, arg_2) {
                (ReducedValue::Boolean(bool_1), ReducedValue::Boolean(bool_2)) => {
                    ReducedValue::Boolean(bool_1 ^ bool_2)
                }
                args @ (ReducedValue::Decimal(_) | ReducedValue::Integer(_), ReducedValue::Decimal(_) | ReducedValue::Integer(_)) => {
                    let int_1 = TryInto::<i128>::try_into(args.0).unwrap();
                    let int_2 = TryInto::<i128>::try_into(args.1).unwrap();
                    ReducedValue::Integer(int_1 ^ int_2)
                }
                _ => return Err("Operator '^' can only be applied between boolean, integers or decimals".to_string()),
            })
        }),
        ("<<", |arg_1, arg_2| {
            Ok(match (arg_1, arg_2) {
                args @ (ReducedValue::Decimal(_) | ReducedValue::Integer(_), ReducedValue::Decimal(_) | ReducedValue::Integer(_)) => {
                    let int_1 = TryInto::<i128>::try_into(args.0).unwrap();
                    let int_2 = TryInto::<i128>::try_into(args.1).unwrap();
                    ReducedValue::Integer(int_1 << int_2)
                }
                _ => return Err("Operator '<<' can only be applied between integers or decimals".to_string()),
            })
        }),
        (">>", |arg_1, arg_2| {
            Ok(match (arg_1, arg_2) {
                args @ (ReducedValue::Decimal(_) | ReducedValue::Integer(_), ReducedValue::Decimal(_) | ReducedValue::Integer(_)) => {
                    let int_1 = TryInto::<i128>::try_into(args.0).unwrap();
                    let int_2 = TryInto::<i128>::try_into(args.1).unwrap();
                    ReducedValue::Integer(int_1 >> int_2)
                }
                _ => return Err("Operator '>>' can only be applied between integers or decimals".to_string()),
            })
        }),
        ("==", |arg_1, arg_2| {
            Ok(ReducedValue::Boolean(arg_1.eq(&arg_2)))
        }),
        ("!=", |arg_1, arg_2| {
            Ok(ReducedValue::Boolean(arg_1.ne(&arg_2)))
        }),
        (">", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(ReducedValue::Boolean(bool_1 > bool_2)),
                              |int_1, int_2| Ok(ReducedValue::Boolean(int_1 > int_2)),
                              |dec_1, dec_2| Ok(ReducedValue::Boolean(dec_1 > dec_2)))
                .map_err(|_| "Operator '>' can only be applied between boolean, integers or decimals".to_string())?
        }),
        ("<", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(ReducedValue::Boolean(bool_1 < bool_2)),
                              |int_1, int_2| Ok(ReducedValue::Boolean(int_1 < int_2)),
                              |dec_1, dec_2| Ok(ReducedValue::Boolean(dec_1 < dec_2)))
                .map_err(|_| "Operator '<' can only be applied between boolean, integers or decimals".to_string())?
        }),
        (">=", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(ReducedValue::Boolean(bool_1 >= bool_2)),
                              |int_1, int_2| Ok(ReducedValue::Boolean(int_1 >= int_2)),
                              |dec_1, dec_2| Ok(ReducedValue::Boolean(dec_1 >= dec_2)))
                .map_err(|_| "Operator '>?' can only be applied between boolean, integers or decimals".to_string())?
        }),
        ("<=", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(ReducedValue::Boolean(bool_1 <= bool_2)),
                              |int_1, int_2| Ok(ReducedValue::Boolean(int_1 <= int_2)),
                              |dec_1, dec_2| Ok(ReducedValue::Boolean(dec_1 <= dec_2)))
                .map_err(|_| "Operator '<=' can only be applied between boolean, integers or decimals".to_string())?
        }),
    ]
}