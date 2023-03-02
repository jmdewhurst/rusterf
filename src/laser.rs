#![warn(clippy::pedantic)]

use std::f32::consts::PI;

use super::ring_buffer::DyadicRingBuffer;

use librp_sys::core;

// TODO: skip logs in debug representation?
#[derive(Debug)]
pub struct Laser {
    wavelength_nm: f32,
    pub input_channel: core::Channel,
    pub output_channel: Option<core::Channel>,
    pub fit_coefficients: [f32; 4],
    fringe_freq: f32,
    pub phase_log: DyadicRingBuffer<f32>,
    pub feedback_log: DyadicRingBuffer<f32>,
}

impl Laser {
    #[must_use]
    pub fn new(n: usize) -> Option<Self> {
        Some(Laser {
            wavelength_nm: 1000.,
            input_channel: core::Channel::CH_1,
            output_channel: None,
            fringe_freq: 1.0,
            fit_coefficients: [0.0, 0.0, 0.0, 0.0],
            phase_log: DyadicRingBuffer::new(n)?,
            feedback_log: DyadicRingBuffer::new(n)?,
        })
    }

    #[inline]
    pub fn set_wavelength(
        &mut self,
        wavelength_nm: f32,
        piezo_scale_factor: f32,
        ramp_amplitude: f32,
    ) {
        self.wavelength_nm = wavelength_nm;
        self.fringe_freq =
            piezo_scale_factor * ramp_amplitude * 2.0 * PI / (16384.0 * wavelength_nm);
    }

    #[inline]
    #[must_use]
    pub fn wavelength_nm(&self) -> f32 {
        self.wavelength_nm
    }

    #[inline]
    #[must_use]
    pub fn fringe_freq(&self) -> f32 {
        self.fringe_freq
    }

    #[inline]
    pub fn append_new_values(&mut self, phase: f32, voltage: f32) {
        self.phase_log.push(phase);
        self.feedback_log.push(voltage);
    }

    /// # Errors
    /// Returns Ok(()) if the resize went through; returns Err(()) if it failed to initialize a new
    /// pair of buffers
    #[allow(clippy::result_unit_err)]
    pub fn resize_logs(&mut self, n_new: usize) -> Result<(), ()> {
        let mut new_phase = DyadicRingBuffer::new(n_new).ok_or(())?;
        let mut new_feedback = DyadicRingBuffer::new(n_new).ok_or(())?;
        new_phase.extend(&self.phase_log);
        new_feedback.extend(&self.feedback_log);
        self.phase_log = new_phase;
        self.feedback_log = new_feedback;
        Ok(())
    }
}
