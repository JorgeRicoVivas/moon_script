use alloc::fmt::{Debug, Formatter};
use alloc::rc::Rc;
use alloc::string::ToString;

use paste::paste;

use crate::execution::RuntimeError;
use crate::value::VBValue;

pub trait ToAbstractFunction<Params, Return, Function, Dummy> {
    fn abstract_function(self) -> VBFunction;
    fn dummy(_params: Params, _return_value: Return, _dummy: Dummy) {}
}

#[derive(Clone)]
pub struct VBFunction {
    function: Rc<dyn Fn(&mut dyn Iterator<Item=Result<VBValue, RuntimeError>>) -> Result<VBValue, RuntimeError>>,
    number_of_params: usize,
}

pub enum VBFunctionExecutingError {
    MissingValue,
    CouldNotParse,
}

impl Debug for VBFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VBFunction")
            .field("function", &"..")
            .field("number_of_parameters", &self.number_of_params)
            .finish()
    }
}

impl VBFunction {
    #[inline]
    pub(crate) fn execute_iter<'values, ValuesIter>(&self, mut values: ValuesIter) -> Result<VBValue, RuntimeError> where ValuesIter: Iterator<Item=Result<VBValue, RuntimeError>> {
        (self.function)(&mut values)
    }

    #[inline]
    pub(crate) fn execute_into_iter<'values, ValuesIter>(&self, values: ValuesIter) -> Result<VBValue, RuntimeError> where ValuesIter: IntoIterator<Item=Result<VBValue, RuntimeError>> {
        (self.function)(&mut values.into_iter())
    }
}

macro_rules! impl_to_wrapped_function {
    (def { n: $params_len:literal names: $($param_names:ident)* }) => {
        paste!{
            impl<$($param_names, [<Error $param_names>], )* TReturn, TFunction, TError: ToString,>
                ToAbstractFunction<($($param_names,)*), TReturn, TFunction, u8> for TFunction
                where $($param_names: TryFrom<VBValue, Error=[<Error $param_names>] > + 'static,)*
                      TReturn: Into<VBValue> + 'static,
                      TFunction: Fn($($param_names),*) -> Result<TReturn,TError> + 'static
            {
                #[allow(unused_mut)]
                #[allow(unused)]
                fn abstract_function(self) -> VBFunction {
                    VBFunction {
                        function: Rc::new(move |values| {
                            $(let paste::item!{[<$param_names:lower>]}  = <$param_names>::try_from(values.next()
                                .ok_or_else(|| RuntimeError::AnArgumentIsMissing{} )??)
                                .map_err(|_| RuntimeError::CannotParseArgument{} )?;)*

                            self($( paste::item!{[<$param_names:lower>]}  ),*)
                                .map(|return_value| return_value.into())
                                .map_err(|err| RuntimeError::FunctionError{ function_error_message:err.to_string() })
                        }),
                        number_of_params: $params_len,
                    }
                }
            }

            impl<$($param_names, [<Error $param_names>], )* TReturn, TFunction>
                ToAbstractFunction<($($param_names,)*), TReturn, TFunction, u16> for TFunction
                where $($param_names: TryFrom<VBValue, Error=[<Error $param_names>]> + 'static,)*
                      TReturn: Into<VBValue> + 'static,
                      TFunction: Fn($($param_names),*) -> TReturn + 'static
            {
                #[allow(unused_mut)]
                #[allow(unused)]
                fn abstract_function(self) -> VBFunction {
                    VBFunction {
                        function: Rc::new(move |values| {
                            $(let paste::item!{[<$param_names:lower>]}  = <$param_names>::try_from(values.next()
                                .ok_or_else(|| RuntimeError::AnArgumentIsMissing{} )??)
                                .map_err(|_| RuntimeError::CannotParseArgument{} )?;)*

                            Ok(self($( paste::item!{[<$param_names:lower>]}  ),*)

                            .into())
                        }),
                        number_of_params: $params_len,
                    }
                }
            }
        }
    };

    ($(def { n: $params_len:literal names: $($param_names:ident)* })*) =>{
        $(impl_to_wrapped_function!{def { n: $params_len names: $($param_names)* }})*
    };
}


impl_to_wrapped_function! {
    def { n: 00 names: }
    def { n: 01 names: PA }
    def { n: 02 names: PA PB }
    def { n: 03 names: PA PB PC }
    def { n: 04 names: PA PB PC PD }
    def { n: 05 names: PA PB PC PD PE }
    def { n: 06 names: PA PB PC PD PE PF }
    def { n: 07 names: PA PB PC PD PE PF PG }
    def { n: 08 names: PA PB PC PD PE PF PG PH }
}

#[cfg(feature = "medium_functions")]
impl_to_wrapped_function! {
    def { n: 09 names: PA PB PC PD PE PF PG PH PI }
    def { n: 10 names: PA PB PC PD PE PF PG PH PI PJ }
    def { n: 11 names: PA PB PC PD PE PF PG PH PI PJ PK }
    def { n: 12 names: PA PB PC PD PE PF PG PH PI PJ PK PL }
    def { n: 13 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM }
    def { n: 14 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN }
    def { n: 15 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO }
    def { n: 16 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP }
}

#[cfg(feature = "big_functions")]
impl_to_wrapped_function! {
    def { n: 17 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ }
    def { n: 18 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR }
    def { n: 19 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS }
    def { n: 20 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU }
    def { n: 21 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV }
    def { n: 22 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT }
    def { n: 23 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW }
    def { n: 24 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX }
}

#[cfg(feature = "massive_functions")]
impl_to_wrapped_function! {
    def { n: 25 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ }
    def { n: 26 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY }
    def { n: 27 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA }
    def { n: 28 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB }
    def { n: 29 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC }
    def { n: 30 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD }
    def { n: 31 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD PAE }
    def { n: 32 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD PAE PAF }
    def { n: 33 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD PAE PAF PAG }
    def { n: 34 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD PAE PAF PAG PAH }
    def { n: 35 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD PAE PAF PAG PAH PAI }
    def { n: 36 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD PAE PAF PAG PAH PAI PAJ }
    def { n: 37 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD PAE PAF PAG PAH PAI PAJ PAK }
    def { n: 38 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD PAE PAF PAG PAH PAI PAJ PAK PAL }
    def { n: 39 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD PAE PAF PAG PAH PAI PAJ PAK PAL PAM }
    def { n: 40 names: PA PB PC PD PE PF PG PH PI PJ PK PL PM PN PO PP PQ PR PS PU PV PT PW PX PZ PY PAA PAB PAC PAD PAE PAF PAG PAH PAI PAJ PAK PAL PAM PAN }
}