#![warn(clippy::pedantic)]

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

use super::circle_buffer::CircleBuffer2n;

use librp_sys::core;

// TODO: skip logs in debug representation?
#[derive(Debug)]
pub struct Laser {
    wavelength_nm: f32,
    pub input_channel: core::Channel,
    pub output_channel: core::Channel,
    pub output_base_offset: f32,
    pub fit_coefficients: [f32; 4],
    fringe_freq: f32,
    pub phase_log: CircleBuffer2n<f32>,
    pub feedback_log: CircleBuffer2n<f32>,
}

impl Laser {
    #[inline]
    pub fn new(n: usize) -> Option<Self> {
        Some(Laser {
            wavelength_nm: 1000.,
            input_channel: core::Channel::CH_1,
            output_channel: core::Channel::CH_1,
            output_base_offset: 0.,
            fringe_freq: 1.0,
            fit_coefficients: [0.0, 0.0, 0.0, 0.0],
            phase_log: CircleBuffer2n::new(n)?,
            feedback_log: CircleBuffer2n::new(n)?,
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
            piezo_scale_factor * ramp_amplitude / (16384.0 * wavelength_nm * 2.0 * PI);
    }

    #[inline]
    pub fn wavelength_nm(&self) -> f32 {
        self.wavelength_nm
    }

    #[inline]
    pub fn fringe_freq(&self) -> f32 {
        self.fringe_freq
    }

    #[inline]
    pub fn append_new_values(&mut self, phase: f32, voltage: f32) {
        self.phase_log.append(phase);
        self.feedback_log.append(voltage);
    }

    pub fn resize_logs(&mut self, n_new: usize) -> Result<(), ()> {
        let mut new_phase = CircleBuffer2n::new(n_new).ok_or(())?;
        let mut new_feedback = CircleBuffer2n::new(n_new).ok_or(())?;
        new_phase.extend(&self.phase_log);
        new_feedback.extend(&self.feedback_log);
        self.phase_log = new_phase;
        self.feedback_log = new_feedback;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct LaserSerialize {
    wavelength_nm: f32,
    input_channel: core::Channel,
    output_channel: core::Channel,
    output_base_offset: f32,
    buffer_length_exponent: usize,
}

impl LaserSerialize {
    fn into_laser(self) -> Laser {
        Laser {
            wavelength_nm: self.wavelength_nm,
            input_channel: self.input_channel,
            output_channel: self.output_channel,
            output_base_offset: self.output_base_offset,
            fit_coefficients: [0.0, 0.0, 0.0, 0.0],
            fringe_freq: 1.0,
            phase_log: CircleBuffer2n::new(self.buffer_length_exponent).unwrap(),
            feedback_log: CircleBuffer2n::new(self.buffer_length_exponent).unwrap(),
        }
    }

    fn from_laser(las: &Laser) -> Self {
        LaserSerialize {
            wavelength_nm: las.wavelength_nm,
            input_channel: las.input_channel,
            output_channel: las.output_channel,
            output_base_offset: las.output_base_offset,
            buffer_length_exponent: las.phase_log.exponent(),
        }
    }
}

impl<'de> Deserialize<'de> for Laser {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(LaserSerialize::deserialize(d)?.into_laser())
    }
}

impl Serialize for Laser {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        LaserSerialize::from_laser(self).serialize(serializer)
    }
}
