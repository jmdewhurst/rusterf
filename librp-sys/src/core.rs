#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![warn(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(clippy::wildcard_imports)]
use enum_primitive::*;

#[cfg(not(any(feature = "no_api", feature = "no_api_loud")))]
include!("bindings.rs");

#[cfg(any(feature = "no_api", feature = "no_api_loud"))]
include!("mock/mod.rs");

macro_rules! wrap_call {
    ($call:ident $(, $arg:expr)* $(,)?) => {
			{match unsafe {
				APIError::from_i32(core:: $call ($($arg,)* ) )
					.unwrap_unchecked()
				} {
				RP_OK => Ok(()),
				err => Err(err),
				}
			}
		}
}
// macro_rules! wrap_call {
//     ($call:ident $(, $arg:expr)* $(,)?) => {
// 			{
// 				print!(
// 					concat!("calling ",  stringify!($call), " "),
// 				);
// 				$(
// 					print!(
// 						"({:?})",
// 						$arg
// 					);
// 				)*
// 				println!("");
// 			{match unsafe {
// 				APIError::from_i32(core:: $call ($($arg,)* ) )
// 					.unwrap_unchecked()
// 				} {
// 				RP_OK => Ok(()),
// 				err => Err(err),
// 				}
// 			}
// 			}
// 		}
// }
// pub(crate) use wrap_call;

enum_from_primitive! {
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum APIError {
    RP_OK = 0, // included for conciseness, but shouldn't be used
    RP_EOED = 1,
    RP_EOMD = 2,
    RP_ECMD = 3,
    RP_EMMD = 4,
    RP_EUMD = 5,
    RP_EOOR = 6,
    RP_ELID = 7,
    RP_EMRO = 8,
    RP_EWIP = 9,
    RP_EPN = 10,
    RP_UIA = 11,
    RP_FCA = 12,
    RP_RCA = 13,
    RP_BTS = 14,
    RP_EIPV = 15,
    RP_EUF = 16,
    RP_ENN = 17,
    RP_EFOB = 18,
    RP_EFCB = 19,
    RP_EABA = 20,
    RP_EFRB = 21,
    RP_EFWB = 22,
    RP_EMNC = 23,
    RP_NOTS = 24,
}
}

pub type APIResult<T> = Result<T, APIError>;

enum_from_primitive! {
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum Channel {
    CH_1 = 0,
    CH_2,
}
}
