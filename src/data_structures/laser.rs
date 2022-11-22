use crate::data_structures::lock::Servo;
pub struct Ref {
    pub wavelength_nm: f32,
    pub lock: Servo,
}
