#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![warn(clippy::pedantic)]
#![warn(clippy::all)]
use crate::core as rp;
use crate::dpin::DigitalPin;
use crate::generator::Generator;
use crate::oscilloscope::Oscilloscope;

#[cfg(not(any(feature = "no_api", feature = "no_api_loud")))]
use std::process::Command;

#[derive(Debug)]
pub enum InitializationError {
    FAILED_TO_LOAD_FPGA_IMAGE,
    API_FAILED(rp::APIError),
}

pub struct Pitaya {
    pub scope: Oscilloscope,
    pub gen: Generator,
    pub dpin: DigitalPin,
}

impl Pitaya {
    /// # Errors
    /// Returns ``FAILED_TO_LOAD_FPGA_IMAGE`` if the program fails to load the Red Pitaya bitmap onto
    /// the FPGA.
    /// If there is a failure in initializing/resetting the API/FPGA, returns one of the ``APIError``
    /// variants indicating the mode of failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    #[cfg(not(any(feature = "no_api", feature = "no_api_loud")))]
    pub fn init() -> Result<Self, InitializationError> {
        use enum_primitive::FromPrimitive;

        let status = Command::new("sh")
            // ensure the FPGA image is loaded onto the Red Pitaya -- otherwise the API is nonsense
            .arg("cat /opt/redpitaya/fpga/fpga_0.94.bit > /dev/xdevcfg")
            .status()
            .map_err(|_| InitializationError::FAILED_TO_LOAD_FPGA_IMAGE)?;
        if !status.success() {
            return Err(InitializationError::FAILED_TO_LOAD_FPGA_IMAGE);
        }

        match rp::APIError::from_i32(unsafe { rp::rp_InitReset(true) })
            .expect("rp_InitReset returned unexpected error code!")
        {
            rp::APIError::RP_OK => Ok(Pitaya {
                scope: Oscilloscope::init(),
                gen: Generator::init(),
                dpin: DigitalPin::init(),
            }),
            err => Err(InitializationError::API_FAILED(err)),
        }
    }

    #[cfg(any(feature = "no_api", feature = "no_api_loud"))]
    pub fn init() -> Result<Self, InitializationError> {
        rp::core_mock_init();
        Ok(Pitaya {
            scope: Oscilloscope::init(),
            gen: Generator::init(),
            dpin: DigitalPin::init(),
        })
    }
}

impl Drop for Pitaya {
    fn drop(&mut self) {
        unsafe { rp::rp_Release() };
    }
}
