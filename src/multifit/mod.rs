#![warn(clippy::pedantic)]
#![allow(clippy::similar_names)]

use std::f32::consts::PI;
use std::ffi::{c_char, c_int, CStr};
use std::os::raw::c_float;
use std::ptr::{self, null_mut};

use chrono::Local;

extern "C" {
    fn init_multifit_setup(setup: *mut FitSetup) -> u32;
    fn release_multifit_resources(setup: *mut FitSetup);
    fn do_fitting(setup: *mut FitSetup, data: DataRaw) -> FitResultRaw;
    fn gsl_strerror(gsl_errno: c_int) -> *const c_char;
}

const LOW_CONTRAST_THRESHOLD: f32 = 100.0;

pub fn wrapped_angle_difference(a: f32, b: f32) -> f32 {
    (a.sin() * b.cos() - a.cos() * b.sin()).atan2(a.cos() * b.cos() + a.sin() * b.sin())
}

pub fn sinusoid(x: f32, p: [f32; 4]) -> f32 {
    p[0] * (p[1] * x - p[2]).cos() + p[3]
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
    niter: c_int,
    params: [f32; 4],
}

#[derive(Debug)]
pub struct FitResult {
    pub gsl_status: i32,
    pub n_iterations: i32,
    pub params: [f32; 4],
    pub low_contrast: bool,
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
    pub xtol: c_float,
    pub gtol: c_float,
    pub ftol: c_float,
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
        let mut raw_result = unsafe { do_fitting(self as *mut FitSetup, data_struct) };
        if raw_result.gsl_status != 0 {
            eprintln!("[{}] fitting error [{}]", Local::now(), unsafe {
                CStr::from_ptr(gsl_strerror(raw_result.gsl_status))
                    .to_str()
                    .expect("the library function gsl_strerror should return a valid C-style string (with static lifetime)")
            });
            println!("{} iterations", raw_result.niter);
        }

        // Do some post-processing to ensure that data are in consistent form. These are not necessarily indications of a bad fit
        if raw_result.params[0] < 0.0 {
            raw_result.params[0] *= -1.0;
            raw_result.params[2] += PI;
        }

        let low_contrast = raw_result.params[0] < LOW_CONTRAST_THRESHOLD;

        FitResult {
            gsl_status: raw_result.gsl_status,
            n_iterations: raw_result.niter as i32,
            params: raw_result.params,
            low_contrast,
        }
    }
}

impl Drop for FitSetup {
    fn drop(&mut self) {
        unsafe { release_multifit_resources(self as *mut FitSetup) };
    }
}
