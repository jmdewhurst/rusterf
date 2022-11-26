use super::laser::Laser;
use super::lock::Servo;
use crate::multifit;

#[derive(Debug)]
pub struct Interferometer {
    pub ref_laser: Laser,
    pub ref_lock: Servo,
    pub slave_laser: Laser,
    pub slave_lock: Servo,
    pub fit_setup: multifit::FitSetup,

    pub piezo_scale_factor: f32,
    pub ramp_amplitude: f32,
    pub preamp_gain: f32,
}
