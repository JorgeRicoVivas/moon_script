use alloc::{format, vec};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::value::MoonValue;

const ARITHMETIC_RESULT_BOOL: u8 = 0;
const ARITHMETIC_RESULT_INT: u8 = 1;
const ARITHMETIC_RESULT_DECIMAL: u8 = 2;

fn arithmetic_choice(arg1: MoonValue, arg2: MoonValue, on_bools: fn(bool, bool) -> Result<MoonValue, String>, on_int: fn(i128, i128) -> Result<MoonValue, String>, on_decimal: fn(f64, f64) -> Result<MoonValue, String>) -> Result<Result<MoonValue, String>, (MoonValue, MoonValue)> {
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

fn arithmetic_result(arg1: &MoonValue, arg2: &MoonValue) -> Option<u8> {
    let top_left_level = match arg1 {
        MoonValue::Boolean(_) => ARITHMETIC_RESULT_BOOL,
        MoonValue::Integer(_) => ARITHMETIC_RESULT_INT,
        MoonValue::Decimal(_) => ARITHMETIC_RESULT_DECIMAL,
        _ => return None,
    };
    let top_right_level = match arg2 {
        MoonValue::Boolean(_) => ARITHMETIC_RESULT_BOOL,
        MoonValue::Integer(_) => ARITHMETIC_RESULT_INT,
        MoonValue::Decimal(_) => ARITHMETIC_RESULT_DECIMAL,
        _ => return None,
    };
    Some(if top_right_level >= top_left_level { top_right_level } else { top_left_level })
}

pub(crate) fn get_unary_operators() -> Vec<(&'static str, fn(MoonValue) -> Result<MoonValue, String>)> {
    vec![
        ("!", |arg| {
            match arg {
                MoonValue::Boolean(bool) => Ok(MoonValue::Boolean(!bool)),
                MoonValue::Integer(int) => Ok(MoonValue::Integer(!int)),
                MoonValue::Null | MoonValue::Decimal(_) | MoonValue::String(_) | MoonValue::Array(_) =>
                    Err("Unary operator '!' only can be applied between booleans or integers".to_string()),
            }
        }),
        ("-", |arg| {
            match arg {
                MoonValue::Integer(int) => Ok(MoonValue::Integer(-int)),
                MoonValue::Decimal(dec) => Ok(MoonValue::Decimal(-dec)),
                MoonValue::Null | MoonValue::Boolean(_) | MoonValue::String(_) | MoonValue::Array(_) =>
                    Err("Unary operator '-' only can be applied between integers or decimals".to_string()),
            }
        }),
    ]
}


pub(crate) fn get_binary_operators() -> Vec<(&'static str, fn(MoonValue, MoonValue) -> Result<MoonValue, String>)> {
    vec![
        ("+", |arg_1, arg_2| {
            match (&arg_1, &arg_2) {
                (MoonValue::String(string_1), MoonValue::String(string_2)) => {
                    return Ok(MoonValue::String(format!("{string_1}{string_2}")));
                }
                (MoonValue::String(string_1), arg_2) => {
                    return Ok(MoonValue::String(format!("{string_1}{arg_2}")));
                }
                (arg_1, MoonValue::String(string_2)) => {
                    return Ok(MoonValue::String(format!("{arg_1}{string_2}")));
                }
                _ => {}
            }

            match arithmetic_choice(arg_1, arg_2,
                                    |bool_1, bool_2| Ok(MoonValue::Boolean(bool_1 || bool_2)),
                                    |int_1, int_2| Ok(MoonValue::Integer(int_1.checked_add(int_2).unwrap_or(i128::MAX))),
                                    |dec_1, dec_2| Ok(MoonValue::Decimal(dec_1 + dec_2))) {
                Ok(res) => { return res; }
                Err((arg_1, arg_2)) => {
                    Ok(match (arg_1, arg_2) {
                        (MoonValue::Array(mut array_1), MoonValue::Array(array_2)) => {
                            array_1.extend(array_2.into_iter());
                            MoonValue::Array(array_1)
                        }
                        _ => return Err("Operator '+' can only be applied between booleans, integers, decimals, arrays or strings".to_string()),
                    })
                }
            }
        }),
        ("-", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(MoonValue::Boolean(bool_1 && !bool_2)),
                              |int_1, int_2| Ok(MoonValue::Integer(int_1.checked_sub(int_2).unwrap_or(i128::MIN))),
                              |dec_1, dec_2| Ok(MoonValue::Decimal(dec_1 - dec_2)))
                .map_err(|_| "Operator '-' can only be applied between booleans, integers or decimals".to_string())?
        }),
        ("*", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(MoonValue::Boolean(bool_1 && bool_2)),
                              |int_1, int_2| Ok(MoonValue::Integer(int_1.checked_mul(int_2).unwrap_or(i128::MAX))),
                              |dec_1, dec_2| Ok(MoonValue::Decimal(dec_1 * dec_2)))
                .map_err(|_| "Operator '*' can only be applied between booleans, integers or decimals".to_string())?
        }),
        ("/", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |_, _| Err("Operator '/' cannot be applied between booleans".to_string()),
                              |int_1, int_2| {
                                  let res = (int_1 as f64) / (int_2 as f64);
                                  if !res.is_normal() {
                                      Ok(MoonValue::Integer(int_1.checked_div(int_2).unwrap_or(i128::MAX)))
                                  } else if res == (res as i128 as f64) {
                                      Ok(MoonValue::Integer(res as i128))
                                  } else {
                                      Ok(MoonValue::Decimal(res))
                                  }
                              },
                              |dec_1, dec_2| Ok(MoonValue::Decimal(dec_1 / dec_2)))
                .map_err(|_| "Operator '/' can only be applied between integers or decimals".to_string())?
        }),
        ("%", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |_, _| Err("Operator '%' cannot be applied between booleans".to_string()),
                              |int_1, int_2| Ok(MoonValue::Integer(int_1.checked_rem(int_2).unwrap_or(0))),
                              |dec_1, dec_2| Ok(MoonValue::Decimal(dec_1 % dec_2)))
                .map_err(|_| "Operator '%' can only be applied between integers or decimals".to_string())?
        }),
        ("&&", |arg_1, arg_2| {
            Ok(match (arg_1, arg_2) {
                (MoonValue::Boolean(bool_1), MoonValue::Boolean(bool_2)) => {
                    MoonValue::Boolean(bool_1 && bool_2)
                }
                args @ (MoonValue::Decimal(_) | MoonValue::Integer(_), MoonValue::Decimal(_) | MoonValue::Integer(_)) => {
                    let int_1 = TryInto::<i128>::try_into(args.0).unwrap();
                    let int_2 = TryInto::<i128>::try_into(args.1).unwrap();
                    MoonValue::Integer(int_1 & int_2)
                }
                _ => return Err("Operator '&&' can only be applied between boolean, integers or decimals".to_string()),
            })
        }),
        ("^", |arg_1, arg_2| {
            Ok(match (arg_1, arg_2) {
                (MoonValue::Boolean(bool_1), MoonValue::Boolean(bool_2)) => {
                    MoonValue::Boolean(bool_1 ^ bool_2)
                }
                args @ (MoonValue::Decimal(_) | MoonValue::Integer(_), MoonValue::Decimal(_) | MoonValue::Integer(_)) => {
                    let int_1 = TryInto::<i128>::try_into(args.0).unwrap();
                    let int_2 = TryInto::<i128>::try_into(args.1).unwrap();
                    MoonValue::Integer(int_1 ^ int_2)
                }
                _ => return Err("Operator '^' can only be applied between boolean, integers or decimals".to_string()),
            })
        }),
        ("<<", |arg_1, arg_2| {
            Ok(match (arg_1, arg_2) {
                args @ (MoonValue::Decimal(_) | MoonValue::Integer(_), MoonValue::Decimal(_) | MoonValue::Integer(_)) => {
                    let int_1 = TryInto::<i128>::try_into(args.0).unwrap();
                    let int_2 = TryInto::<i128>::try_into(args.1).unwrap();
                    MoonValue::Integer(int_1 << int_2)
                }
                _ => return Err("Operator '<<' can only be applied between integers or decimals".to_string()),
            })
        }),
        (">>", |arg_1, arg_2| {
            Ok(match (arg_1, arg_2) {
                args @ (MoonValue::Decimal(_) | MoonValue::Integer(_), MoonValue::Decimal(_) | MoonValue::Integer(_)) => {
                    let int_1 = TryInto::<i128>::try_into(args.0).unwrap();
                    let int_2 = TryInto::<i128>::try_into(args.1).unwrap();
                    MoonValue::Integer(int_1 >> int_2)
                }
                _ => return Err("Operator '>>' can only be applied between integers or decimals".to_string()),
            })
        }),
        ("==", |arg_1, arg_2| {
            Ok(MoonValue::Boolean(arg_1.eq(&arg_2)))
        }),
        ("!=", |arg_1, arg_2| {
            Ok(MoonValue::Boolean(arg_1.ne(&arg_2)))
        }),
        (">", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(MoonValue::Boolean(bool_1 > bool_2)),
                              |int_1, int_2| Ok(MoonValue::Boolean(int_1 > int_2)),
                              |dec_1, dec_2| Ok(MoonValue::Boolean(dec_1 > dec_2)))
                .map_err(|_| "Operator '>' can only be applied between boolean, integers or decimals".to_string())?
        }),
        ("<", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(MoonValue::Boolean(bool_1 < bool_2)),
                              |int_1, int_2| Ok(MoonValue::Boolean(int_1 < int_2)),
                              |dec_1, dec_2| Ok(MoonValue::Boolean(dec_1 < dec_2)))
                .map_err(|_| "Operator '<' can only be applied between boolean, integers or decimals".to_string())?
        }),
        (">=", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(MoonValue::Boolean(bool_1 >= bool_2)),
                              |int_1, int_2| Ok(MoonValue::Boolean(int_1 >= int_2)),
                              |dec_1, dec_2| Ok(MoonValue::Boolean(dec_1 >= dec_2)))
                .map_err(|_| "Operator '>?' can only be applied between boolean, integers or decimals".to_string())?
        }),
        ("<=", |arg_1, arg_2| {
            arithmetic_choice(arg_1, arg_2,
                              |bool_1, bool_2| Ok(MoonValue::Boolean(bool_1 <= bool_2)),
                              |int_1, int_2| Ok(MoonValue::Boolean(int_1 <= int_2)),
                              |dec_1, dec_2| Ok(MoonValue::Boolean(dec_1 <= dec_2)))
                .map_err(|_| "Operator '<=' can only be applied between boolean, integers or decimals".to_string())?
        }),
    ]
}