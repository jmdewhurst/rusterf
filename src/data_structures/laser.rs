use crate::data_structures::lock::Servo;
pub struct Ref {
    pub wavelength_nm: f32,
    pub lock: Servo,

    pub ramp_amplitude_V: f32,
}
