#![warn(clippy::pedantic)]
#![allow(clippy::similar_names)]

use std::ffi::c_int;
use std::ptr::{self, null_mut};

extern "C" {
    fn init_multifit_setup(setup: *mut FitSetup) -> u32;
    fn release_multifit_resources(setup: *mut FitSetup);
    fn do_fitting(setup: *mut FitSetup, data: DataRaw) -> FitResultRaw;
}

#[derive(Debug)]
#[repr(C)]
struct DataRaw {
    num_points: u32,
    skip_rate: u32,
    y: *const f32,
    guess: [f32; 4],
}

#[derive(Debug)]
#[repr(C)]
struct FitResultRaw {
    gsl_status: c_int,
    params: [f32; 4],
}

#[derive(Debug)]
pub struct FitResult {
    gsl_status: i32,
    params: [f32; 4],
}

// opaque structs handled on the C side
enum Workspace {}
enum MultifitFDF {}
enum GslMultifitParameters {}
enum GslVector {}

#[derive(Debug)]
#[repr(C)]
pub struct FitSetup {
    work: *mut Workspace, // Parameters handled in the C library
    fdf: *mut MultifitFDF,
    setup_params: *mut GslMultifitParameters,
    guess: *mut GslVector,
    pub skip_rate: u32,
    pub num_points: u32,
    pub max_iterations: u32,
    pub xtol: f32,
    pub gtol: f32,
    pub ftol: f32,
    pub max_av_ratio: f32,
}

impl FitSetup {
    pub fn init(
        skip_rate: u32,
        num_points: u32,
        max_iterations: u32,
        xtol: f32,
        gtol: f32,
        ftol: f32,
        max_av_ratio: f32,
    ) -> Option<Self> {
        let mut setup = FitSetup {
            work: null_mut(),
            fdf: null_mut(),
            setup_params: null_mut(),
            guess: null_mut(),
            skip_rate,
            num_points,
            max_iterations,
            xtol,
            gtol,
            ftol,
            max_av_ratio,
        };
        match unsafe { init_multifit_setup(ptr::addr_of_mut!(setup)) } {
            0 => Some(setup),
            _ => None,
        }
    }

    pub fn fit(&mut self, data: &[f32], guess: [f32; 4]) -> FitResult {
        let data_struct = DataRaw {
            num_points: self.num_points,
            skip_rate: self.skip_rate,
            y: data.as_ptr(),
            guess,
        };
        let raw_result = unsafe { do_fitting(self as *mut FitSetup, data_struct) };
        FitResult {
            gsl_status: raw_result.gsl_status,
            params: raw_result.params,
        }
    }
}

impl Drop for FitSetup {
    fn drop(&mut self) {
        unsafe { release_multifit_resources(self as *mut FitSetup) };
    }
}
