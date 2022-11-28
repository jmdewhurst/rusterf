#![warn(clippy::pedantic)]

use std::f32::consts::PI;

use super::circle_buffer::CircleBuffer2n;

use librp_sys::core;

#[derive(Debug)]
pub struct Laser {
    wavelength_nm: f32,
    pub input_channel: core::Channel,
    pub output_channel: core::Channel,
    pub output_base_offset: f32,
    pub phase_log: CircleBuffer2n<f32>,
    pub feedback_log: CircleBuffer2n<f32>,
    fringe_freq: f32,
}

impl Laser {
    pub fn new(n: usize) -> Option<Self> {
        Some(Laser {
            wavelength_nm: 1000.,
            input_channel: core::Channel::CH_1,
            output_channel: core::Channel::CH_1,
            output_base_offset: 0.,
            phase_log: CircleBuffer2n::new(n)?,
            feedback_log: CircleBuffer2n::new(n)?,
            fringe_freq: 1.0,
        })
    }

    pub fn set_wavelength(
        &mut self,
        wavelength_nm: f32,
        piezo_scale_factor: f32,
        ramp_amplitude: f32,
    ) {
        self.wavelength_nm = wavelength_nm;
        self.fringe_freq =
            piezo_scale_factor * ramp_amplitude / (16384.0 * wavelength_nm * 2.0 * PI);
    }

    pub fn wavelength_nm(&self) -> f32 {
        self.wavelength_nm
    }

    pub fn append_new_values(&mut self, phase: f32, voltage: f32) {
        self.phase_log.append(phase);
        self.feedback_log.append(voltage);
    }

    pub fn resize_logs(&mut self, n_new: usize) -> Result<(), ()> {
        let opt_phase = CircleBuffer2n::new(n_new);
        let opt_feedback = CircleBuffer2n::new(n_new);
        if !(opt_phase.is_some() && opt_feedback.is_some()) {
            return Err(());
        };
        let mut new_phase = opt_phase.unwrap();
        new_phase.extend(&self.phase_log);
        let mut new_feedback = opt_feedback.unwrap();
        new_feedback.extend(&self.feedback_log);
        self.phase_log = new_phase;
        self.feedback_log = new_feedback;
        Ok(())
    }
}
