#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![warn(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
use crate::core;
use crate::core::{APIError, APIError::RP_OK, APIResult};
use enum_primitive::*;
// use std::marker::PhantomData;
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
impl std::str::FromStr for Pin {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "LED_0" => Ok(Pin::LED_0),
            "LED_1" => Ok(Pin::LED_1),
            "LED_2" => Ok(Pin::LED_2),
            "LED_3" => Ok(Pin::LED_3),
            "LED_4" => Ok(Pin::LED_4),
            "LED_5" => Ok(Pin::LED_5),
            "LED_6" => Ok(Pin::LED_6),
            "LED_7" => Ok(Pin::LED_7),
            "DIO0_P" => Ok(Pin::DIO0_P),
            "DIO1_P" => Ok(Pin::DIO1_P),
            "DIO2_P" => Ok(Pin::DIO2_P),
            "DIO3_P" => Ok(Pin::DIO3_P),
            "DIO4_P" => Ok(Pin::DIO4_P),
            "DIO5_P" => Ok(Pin::DIO5_P),
            "DIO6_P" => Ok(Pin::DIO6_P),
            "DIO7_P" => Ok(Pin::DIO7_P),
            "DIO0_N" => Ok(Pin::DIO0_N),
            "DIO1_N" => Ok(Pin::DIO1_N),
            "DIO2_N" => Ok(Pin::DIO2_N),
            "DIO3_N" => Ok(Pin::DIO3_N),
            "DIO4_N" => Ok(Pin::DIO4_N),
            "DIO5_N" => Ok(Pin::DIO5_N),
            "DIO6_N" => Ok(Pin::DIO6_N),
            "DIO7_N" => Ok(Pin::DIO7_N),
            _ => Err(()),
        }
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

#[derive(Debug)]
pub struct DigitalPin {
    _intern: (),
}

impl DigitalPin {
    pub(crate) fn init() -> Self {
        DigitalPin { _intern: () }
    }

    pub fn set_direction(&mut self, pin: Pin, dir: PinDirection) -> APIResult<()> {
        wrap_call!(
            rp_DpinSetDirection,
            pin as core::rp_dpin_t,
            dir as core::rp_pinDirection_t
        )
    }
    pub fn set_state(&mut self, pin: Pin, val: PinState) -> APIResult<()> {
        wrap_call!(
            rp_DpinSetState,
            pin as core::rp_dpin_t,
            val as core::rp_pinState_t
        )
    }

    pub fn get_state(&mut self, pin: Pin) -> APIResult<PinState> {
        let mut pin_state = 0;
        wrap_call!(
            rp_DpinGetState,
            pin as core::rp_dpin_t,
            std::ptr::addr_of_mut!(pin_state),
        )?;
        Ok(unsafe { PinState::from_u32(pin_state).unwrap_unchecked() })
    }
}
