#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![warn(clippy::pedantic)]
#![warn(clippy::all)]
use crate::core::{APIError, APIError::RP_OK, APIResult};
use crate::{core, pitaya};
use enum_primitive::*;
use std::marker::PhantomData;
// use std::mem::MaybeUninit;

enum_from_primitive! {
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum Pin {
        LED_0 = 0,
        LED_1,
        LED_2,
        LED_3,
        LED_4,
        LED_5,
        LED_6,
        LED_7,
        DIO0_P,
        DIO1_P,
        DIO2_P,
        DIO3_P,
        DIO4_P,
        DIO5_P,
        DIO6_P,
        DIO7_P,
        DIO0_N,
        DIO1_N,
        DIO2_N,
        DIO3_N,
        DIO4_N,
        DIO5_N,
        DIO6_N,
        DIO7_N,
}
}
enum_from_primitive! {
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum PinState {
        Low = 0,
        High,
}
}
enum_from_primitive! {
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum PinDirection {
        In = 0,
        Out,
}
}

pub struct DigitalPin<'a> {
    phantom: PhantomData<&'a pitaya::Pitaya>,
}

impl<'a> DigitalPin<'a> {
    #[must_use]
    pub fn init(_: &'a pitaya::Pitaya) -> Self {
        DigitalPin {
            phantom: PhantomData,
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_direction(&mut self, pin: Pin, dir: PinDirection) -> APIResult<()> {
        match APIError::from_i32(unsafe {
            core::rp_DpinSetDirection(pin as core::rp_dpin_t, dir as core::rp_pinDirection_t)
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_state(&mut self, pin: Pin, val: PinState) -> APIResult<()> {
        match APIError::from_i32(unsafe {
            core::rp_DpinSetDirection(pin as core::rp_dpin_t, val as core::rp_pinState_t)
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }
}
