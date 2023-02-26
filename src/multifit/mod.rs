#![warn(clippy::pedantic)]
#![allow(clippy::similar_names)]
#![allow(dead_code)]

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

#[must_use]
pub fn wrapped_angle_difference(a: f32, b: f32) -> f32 {
    (a.sin() * b.cos() - a.cos() * b.sin()).atan2(a.cos() * b.cos() + a.sin() * b.sin())
}

#[must_use]
pub fn sinusoid(x: f32, p: [f32; 4]) -> f32 {
    p[0] * (p[1] * x - p[2]).cos() + p[3]
}

#[must_use]
pub fn sinusoid_b(x: f32, p: [f32; 4]) -> f32 {
    p[0] * (p[2] * x).cos() + p[1] * (p[2] * x).cos() + p[3]
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
unsafe impl Send for FitSetup {}

impl FitSetup {
    #[must_use]
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
    /// Guess should be the coefficients to the function
    /// A cos(wx - phi) + offset
    /// Will return coefficients in the same form. If there's no reasonable guess for phi,
    /// then you may have better results setting the guessed value of A to zero.
    /// Internally, this function converts those into
    /// A cos(wx) + B sin(wx) + offset
    /// From the user's perspective, this should function as if it fit the first function above, but
    /// the code on the C side MUST use the second function.
    /// # Panics
    /// If building with `debug_assertions`, i.e. a development build, will panic if you try to fit
    /// data of different length than the configured `FitSetup`
    #[allow(clippy::cast_precision_loss)]
    pub fn fit(&mut self, data: &[f32], guess: [f32; 4]) -> FitResult {
        if cfg!(debug_assertions) {
            assert!(
                data.len() == self.num_points as usize,
                "Cannot fit to data of length != configured number of points"
            );
        } else if data.len() != self.num_points as usize {
            let data = &data[..data.len().min(self.num_points as usize)];
            eprintln!("[{}] function multifit::fit recieved data of length {} not equal to the configured length {}", Local::now(), data.len(), self.num_points);
        }
        let guess_internal = [
            guess[0] * guess[2].cos(),
            guess[0] * guess[2].sin(),
            guess[1] * self.skip_rate as f32,
            guess[3],
        ];
        let data_struct = DataRaw {
            num_points: self.num_points,
            skip_rate: self.skip_rate,
            y: data.as_ptr(),
            guess: guess_internal,
        };
        let raw_result = unsafe { do_fitting(self as *mut FitSetup, data_struct) };
        if raw_result.gsl_status != 0 {
            eprintln!("[{}] fitting error [{}]", Local::now(), unsafe {
                CStr::from_ptr(gsl_strerror(raw_result.gsl_status))
                    .to_str()
                    .expect("the library function gsl_strerror should return a valid C-style string (with static lifetime)")
            });
            eprintln!("{} iterations", raw_result.niter);
        }

        let params = [
            (raw_result.params[0] * raw_result.params[0]
                + raw_result.params[1] * raw_result.params[1])
                .sqrt(),
            raw_result.params[2] / self.skip_rate as f32,
            raw_result.params[1].atan2(raw_result.params[0]),
            raw_result.params[3],
        ];

        let low_contrast = params[0] < LOW_CONTRAST_THRESHOLD;

        FitResult {
            gsl_status: raw_result.gsl_status,
            n_iterations: raw_result.niter,
            params,
            low_contrast,
        }
    }

    /// # Panics
    /// panics if passed data of different length that the configured length of `self`
    pub fn fit_deprecated(&mut self, data: &[f32], guess: [f32; 4]) -> FitResult {
        // function configured to fit the function A * cos(w x - phi) + offset
        // Not as computationally stable as the newer one, but leaving it in for posterity
        assert!(
            data.len() == self.num_points as usize,
            "Cannot fit to data of length != configured number of points"
        );
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
            eprintln!("{} iterations", raw_result.niter);
        }

        // Do some post-processing to ensure that data are in consistent form. These are not necessarily indications of a bad fit
        if raw_result.params[0] < 0.0 {
            raw_result.params[0] *= -1.0;
            raw_result.params[2] += PI;
        }

        let low_contrast = raw_result.params[0] < LOW_CONTRAST_THRESHOLD;

        FitResult {
            gsl_status: raw_result.gsl_status,
            n_iterations: raw_result.niter,
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

// #[cfg(test)]
// mod tests {
//     use rand::Rng;

//     use super::*;

//     #[test]
//     fn basic() {
//         let num_points = 1000;
//         let mut rng = rand::thread_rng();
//         let mut setup = FitSetup::init(1, num_points, 32, 1.0e-8, 1.0e-8, 1.0e-8, 1.5).unwrap();
//         let mut data = Vec::new();
//         let center = [1000.0, 0.02, 0.0, 2000.0];

//         for _ in 0..100 {
//             let actual = [
//                 center[0] * rng.gen_range(0.8..1.2),
//                 center[1] * rng.gen_range(0.9..1.1),
//                 rng.gen_range(-PI..PI),
//                 center[3] + rng.gen_range(-100.0..100.0),
//             ];
//             data.clear();
//             data.extend((0..num_points).map(|x| sinusoid(x as f32, actual)));

//             let res = setup.fit(data.as_slice(), center);
//             assert!((res.params[0] - actual[0]).abs() < 1.0);
//             assert!((res.params[1] - actual[1]).abs() / actual[1] < 0.001);
//             assert!((res.params[2] - actual[2]).abs() < 0.001);
//             assert!((res.params[3] - actual[3]).abs() < 1.0);
//         }
//     }

//     #[test]
//     fn skip_rate() {
//         let num_points = 16384;
//         let center = [1000.0, 0.0012, 0.0, 2000.0];
//         let guess = [1001.0, 0.0011, 0.2, 1900.0];
//         let base_data: Vec<f32> = (0..num_points)
//             .map(|x| sinusoid(x as f32, center))
//             .collect();
//         for skip_rate in [1u32, 2, 4, 8, 10, 40, 100, 1000] {
//             let num_points_reduced = (num_points + skip_rate - 1) / skip_rate;
//             let data_reduced: Vec<f32> = base_data
//                 .iter()
//                 .copied()
//                 .step_by(skip_rate as usize)
//                 .collect();
//             let mut setup = FitSetup::init(
//                 skip_rate,
//                 num_points_reduced,
//                 32,
//                 1.0e-8,
//                 1.0e-8,
//                 1.0e-8,
//                 1.5,
//             )
//             .unwrap();
//             let res = setup.fit(data_reduced.as_slice(), guess);
//             assert!((res.params[0] - center[0]).abs() < 1.0);
//             assert!((res.params[1] - center[1]).abs() / center[1] < 0.001);
//             assert!((res.params[2] - center[2]).abs() < 0.001);
//             assert!((res.params[3] - center[3]).abs() < 1.0);
//         }
//     }

//     #[test]
//     fn iterations() {
//         let num_points = 1000;
//         let mut rng = rand::thread_rng();
//         let mut setup = FitSetup::init(1, num_points, 32, 1.0e-8, 1.0e-8, 1.0e-8, 1.5).unwrap();
//         let mut data = Vec::new();
//         let center = [1000.0, 0.02, 0.0, 2000.0];

//         for _ in 0..100 {
//             let actual = [
//                 center[0] * rng.gen_range(0.8..1.2),
//                 center[1] * rng.gen_range(0.9..1.1),
//                 rng.gen_range(-PI..PI),
//                 center[3] + rng.gen_range(-100.0..100.0),
//             ];
//             data.clear();
//             data.extend((0..num_points).map(|x| sinusoid(x as f32, actual)));

//             let res = setup.fit(data.as_slice(), center);
//             assert!((res.params[0] - actual[0]).abs() < 1.0);
//             assert!((res.params[1] - actual[1]).abs() / actual[1] < 0.001);
//             assert!((res.params[2] - actual[2]).abs() < 0.001);
//             assert!((res.params[3] - actual[3]).abs() < 1.0);
//             assert!(res.n_iterations < 16);
//         }
//     }

//     #[test]
//     fn stability() {
//         let num_points = 100;
//         let num_trials = 10_000;
//         let mut rng = rand::thread_rng();
//         let mut setup = FitSetup::init(1, num_points, 32, 1.0e-8, 1.0e-8, 1.0e-8, 1.5).unwrap();
//         let mut data = Vec::new();
//         let center = [1000.0, 0.2, 0.0, 2000.0];

//         let mut num_failures = 0;
//         for i in 0..num_trials {
//             let actual = [
//                 center[0] * rng.gen_range(0.2..1.5),
//                 center[1] * rng.gen_range(0.8..1.25),
//                 rng.gen_range(-PI..PI),
//                 center[3] + rng.gen_range(-1000.0..1000.0),
//             ];
//             data.clear();
//             data.extend((0..num_points).map(|x| sinusoid(x as f32, actual)));

//             let res = setup.fit(data.as_slice(), [0.0, center[1], 0.0, 0.0]);
//             if ((res.params[0] - actual[0]).abs() > 1.0)
//                 || ((res.params[1] - actual[1]).abs() / actual[1] > 0.001)
//                 || ((res.params[2] - actual[2]).abs() > 0.001)
//                 || ((res.params[3] - actual[3]).abs() > 1.0)
//             {
//                 println!("failure at iteration {}:", i);
//                 println!("fitting {:?}\nresults {:?}", actual, res.params);
//                 println!("guess {:?}", center);
//                 num_failures += 1;
//             }

//             if num_failures > 5 {
//                 println!(
//                     "failed {} times in {} attempts, exceeding threshold of {}",
//                     num_failures, num_trials, 5
//                 );
//                 panic!();
//             }
//             // assert!((res.params[0] - actual[0]).abs() < 1.0);
//             // assert!((res.params[1] - actual[1]).abs() / actual[1] < 0.001);
//             // assert!((res.params[2] - actual[2]).abs() < 0.001);
//             // assert!((res.params[3] - actual[3]).abs() < 1.0);
//         }
//     }
// }
