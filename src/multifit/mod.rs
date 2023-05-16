#![warn(clippy::pedantic)]
#![allow(clippy::similar_names)]
// For some reason, rust-analyzer marks functions as ''dead code'' if they're unused in the library
// crate, even if they're publicly exported and used in the binary crate.
#![allow(dead_code)]

use std::ffi::{c_char, c_int};
use std::num::NonZeroU32;
use std::os::raw::c_float;
use std::ptr::{self, null_mut};

extern "C" {
    fn init_multifit_setup(setup: *mut FitSetup) -> u32;
    fn release_multifit_resources(setup: *mut FitSetup);
    fn do_fitting(setup: *mut FitSetup, data: DataRaw) -> FitResultRaw;
    fn gsl_strerror(gsl_errno: c_int) -> *const c_char;
}

#[must_use]
pub fn wrapped_angle_difference(a: f32, b: f32) -> f32 {
    (a.sin() * b.cos() - a.cos() * b.sin()).atan2(a.cos() * b.cos() + a.sin() * b.sin())
}

#[must_use]
pub fn sinusoid(x: f32, p: [f32; 4]) -> f32 {
    p[0] * (p[1] * x - p[2]).cos() + p[3]
}

#[derive(Debug)]
#[repr(C)]
struct DataRaw {
    num_points: u32,
    skip_rate: u32,
    y: *const f32,
    guess: [f32; 5],
}

#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
struct FitResultRaw {
    gsl_status: c_int,
    niter: c_int,
    params: [f32; 5],
    param_errs_raw: [f32; 5],
    chisq: f32,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct FitResult {
    pub gsl_status: i32,
    pub n_iterations: i32,
    pub params: [f32; 5],
    pub param_errs: [f32; 5],
    pub low_contrast: bool,
    pub chisq: f32,
    pub dof: u32,
    pub invalid_params: bool,
}

impl FitResult {
    #[inline]
    #[must_use]
    pub fn reduced_chisq(&self) -> f32 {
        self.chisq / self.dof as f32
    }
}

// opaque structs handled on the C side
enum Workspace {}
enum MultifitFDF {}
enum GslMultifitParameters {}
enum GslVector {}
enum GslMatrix {}

#[derive(Debug)]
#[repr(C)]
pub struct FitSetup {
    work: *mut Workspace, // Parameters handled in the C library
    fdf: *mut MultifitFDF,
    setup_params: *mut GslMultifitParameters,
    guess: *mut GslVector,
    covariance: *mut GslMatrix,
    pub skip_rate: u32,
    pub num_points: u32,
    pub max_iterations: u32,
    pub xtol: c_float,
    pub gtol: c_float,
    pub ftol: c_float,
    pub max_av_ratio: f32,
    pub low_contrast_threshold: f32,
}
unsafe impl Send for FitSetup {}

#[derive(Debug, Default, Copy, Clone)]
pub struct Builder {
    skip_rate: Option<NonZeroU32>,
    num_points: u32,
    max_iterations: Option<u32>,
    xtol: Option<f32>,
    gtol: Option<f32>,
    ftol: Option<f32>,
    max_av_ratio: Option<f32>,
    low_contrast_threshold: Option<f32>,
}

impl Builder {
    pub fn stride(&mut self, stride: u32) -> &mut Self {
        self.skip_rate = NonZeroU32::new(stride);
        self
    }
    pub fn max_iterations(&mut self, max_iterations: u32) -> &mut Self {
        self.max_iterations = Some(max_iterations);
        self
    }
    pub fn xtol(&mut self, xtol: f32) -> &mut Self {
        self.xtol = Some(xtol);
        self
    }
    pub fn gtol(&mut self, gtol: f32) -> &mut Self {
        self.gtol = Some(gtol);
        self
    }
    pub fn ftol(&mut self, ftol: f32) -> &mut Self {
        self.ftol = Some(ftol);
        self
    }
    pub fn max_av_ratio(&mut self, max_av_ratio: f32) -> &mut Self {
        self.max_av_ratio = Some(max_av_ratio);
        self
    }
    pub fn low_contrast_threshold(&mut self, low_contrast_threshold: f32) -> &mut Self {
        self.low_contrast_threshold = Some(low_contrast_threshold);
        self
    }
    pub fn opt_stride(&mut self, stride: Option<NonZeroU32>) -> &mut Self {
        self.skip_rate = stride;
        self
    }
    pub fn opt_max_iterations(&mut self, max_iterations: Option<u32>) -> &mut Self {
        self.max_iterations = max_iterations;
        self
    }
    pub fn opt_xtol(&mut self, xtol: Option<f32>) -> &mut Self {
        self.xtol = xtol;
        self
    }
    pub fn opt_gtol(&mut self, gtol: Option<f32>) -> &mut Self {
        self.gtol = gtol;
        self
    }
    pub fn opt_ftol(&mut self, ftol: Option<f32>) -> &mut Self {
        self.ftol = ftol;
        self
    }
    pub fn opt_max_av_ratio(&mut self, max_av_ratio: Option<f32>) -> &mut Self {
        self.max_av_ratio = max_av_ratio;
        self
    }
    pub fn opt_low_contrast_threshold(&mut self, low_contrast_threshold: Option<f32>) -> &mut Self {
        self.low_contrast_threshold = low_contrast_threshold;
        self
    }
    #[must_use]
    pub fn init(&self) -> Option<FitSetup> {
        let mut setup = FitSetup {
            work: null_mut(),
            fdf: null_mut(),
            setup_params: null_mut(),
            guess: null_mut(),
            covariance: null_mut(),
            skip_rate: self.skip_rate.map(NonZeroU32::get).unwrap_or(1),
            num_points: self.num_points,
            max_iterations: self.max_iterations.unwrap_or(16),
            xtol: self.xtol.unwrap_or(1.0e-8),
            gtol: self.gtol.unwrap_or(1.0e-8),
            ftol: self.ftol.unwrap_or(1.0e-8),
            max_av_ratio: self.max_av_ratio.unwrap_or(0.75),
            low_contrast_threshold: self.low_contrast_threshold.unwrap_or(100.0),
        };
        match unsafe { init_multifit_setup(ptr::addr_of_mut!(setup)) } {
            0 => Some(setup),
            _ => None,
        }
    }
}

impl FitSetup {
    #[must_use]
    pub fn new(num_points: u32) -> Builder {
        Builder {
            num_points,
            ..Default::default()
        }
    }
    /// Guess should be the coefficients to the function
    /// A cos(wx + alpha x x - phi) + offset
    /// Will return coefficients in the same form. If there's no reasonable guess for phi,
    /// then you may have better results setting the guessed value of A to zero.
    /// Internally, this function converts those into
    /// A cos(wx + alpha x x) + B sin(wx + alpha x x) + offset
    /// From the user's perspective, this should function as if it fit the first function above, but
    /// the code on the C side MUST use the second function.
    /// # Panics
    /// If building with `debug_assertions`, i.e. a development build, will panic if you try to fit
    /// data of different length than the configured `FitSetup`
    #[allow(clippy::cast_precision_loss)]
    pub fn fit_five_parameter(&mut self, data: &[f32], guess: [f32; 5]) -> FitResult {
        assert!(
            data.len() == self.num_points as usize,
            "Cannot fit to data of length != configured number of points"
        );
        let guess_internal = [
            guess[0] * guess[2].cos(),
            guess[0] * guess[2].sin(),
            guess[1] * self.skip_rate as f32,
            guess[3] * (self.skip_rate * self.skip_rate) as f32,
            guess[4],
        ];
        let data_struct = DataRaw {
            num_points: self.num_points,
            skip_rate: self.skip_rate,
            y: data.as_ptr(),
            guess: guess_internal,
        };
        let raw_result = unsafe { do_fitting(self as *mut FitSetup, data_struct) };

        let mut params = [
            (raw_result.params[0] * raw_result.params[0]
                + raw_result.params[1] * raw_result.params[1])
                .sqrt(),
            raw_result.params[2] / self.skip_rate as f32,
            raw_result.params[1].atan2(raw_result.params[0]),
            raw_result.params[3] / (self.skip_rate * self.skip_rate) as f32,
            raw_result.params[4],
        ];

        let param_errs = raw_result
            .param_errs_raw
            .map(|x| (x * raw_result.chisq / (self.num_points - 5) as f32).sqrt());

        if params[1] < 0.0 {
            params[1] *= -1.0;
            params[2] *= -1.0;
        }

        let low_contrast = params[0] < self.low_contrast_threshold;

        let mut invalid = false;
        for i in &raw_result.params {
            if i.is_nan() || i.is_infinite() {
                invalid = true;
            };
        }

        FitResult {
            gsl_status: raw_result.gsl_status,
            n_iterations: raw_result.niter,
            params,
            param_errs,
            low_contrast,
            chisq: raw_result.chisq,
            dof: self.num_points - 5,
            invalid_params: invalid,
        }
    }
}

impl Drop for FitSetup {
    fn drop(&mut self) {
        unsafe { release_multifit_resources(self as *mut FitSetup) };
    }
}
