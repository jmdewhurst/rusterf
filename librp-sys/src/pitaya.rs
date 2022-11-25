#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![warn(clippy::pedantic)]
#![warn(clippy::all)]
use crate::core as rp;
use enum_primitive::FromPrimitive;
use std::process::Command;

use crate::resources;

pub enum InitializationError {
    FAILED_TO_LOAD_FPGA_IMAGE,
    API_FAILED(rp::APIError),
}

pub struct Pitaya {
    pub(crate) scope_resource: resources::ScopeResource,
    pub(crate) generator_resource: resources::GeneratorResource,
    pub(crate) dpin_resource: resources::DPinResource,
}

impl Pitaya {
    /// # Errors
    /// Returns ``FAILED_TO_LOAD_FPGA_IMAGE`` if the program fails to load the Red Pitaya bitmap onto
    /// the FPGA.
    /// If there is a failure in initializing/resetting the API/FPGA, returns one of the ``APIError``
    /// variants indicating the mode of failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn init() -> Result<Self, InitializationError> {
        match Command::new("sh")
            // ensure the FPGA image is loaded onto the Red Pitaya -- otherwise the API is nonsense
            .arg("cat /opt/redpitaya/fpga/fpga_0.94.bit > /dev/xdevcfg")
            .status()
        {
            Ok(status) => {
                if !status.success() {
                    return Err(InitializationError::FAILED_TO_LOAD_FPGA_IMAGE);
                }
            }
            Err(_) => return Err(InitializationError::FAILED_TO_LOAD_FPGA_IMAGE),
        }

        if let Some(errcode) = rp::APIError::from_i32(unsafe { rp::rp_InitReset(true) }) {
            match errcode {
                rp::APIError::RP_OK => Ok(Pitaya {
                    scope_resource: resources::ScopeResource {},
                    generator_resource: resources::GeneratorResource {},
                    dpin_resource: resources::DPinResource {},
                }),
                _ => Err(InitializationError::API_FAILED(errcode)),
            }
        } else {
            panic!("rp_InitReset returned unexpected error code!");
        }
    }
}

impl Drop for Pitaya {
    fn drop(&mut self) {
        unsafe { rp::rp_Release() };
    }
}
