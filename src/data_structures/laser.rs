use crate::data_structures::circle_buffer::CircleBuffer2n;

use librp_sys::core::Channel;

#[derive(Debug)]
pub struct Laser {
    pub wavelength_nm: f32,
    pub input_channel: Channel,
    pub output_channel: Channel,
    pub output_base_offset: f32,
    pub phase_log: CircleBuffer2n<f32>,
    pub feedback_log: CircleBuffer2n<f32>,
}

impl Laser {
    pub fn append_new_values(&mut self, phase: f32, voltage: f32) {
        self.phase_log.append(phase);
        self.feedback_log.append(voltage);
    }
}
