#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]
#![allow(clippy::module_name_repetitions)]
use std::f32::consts::PI;

use librp_sys::core::{ADC_SAMPLE_RATE};
use librp_sys::generator::{
    Channel, ChannelBuilder, ChannelInitializationError, Pulse, RawChannel, DC,
};
use librp_sys::oscilloscope::Oscilloscope;

#[derive(Debug)]
pub struct DaqSetup {
    decimation: u32,
    symmetry: f32,
    pub ramp_period_s: f32,
    rise_time_ns: u128,
    pub amplitude_volts: f32,
    pub piezo_scale_factor: f32, // units of nm / Volt
    pub piezo_settle_time_ms: f32,

    ramp_period_us: u64,
    piezo_settle_time_us: u64,

    slave_default_offset_v: Option<f32>,
}

impl DaqSetup {
    #[must_use]
    pub fn new() -> Self {
        DaqSetup {
            decimation: 1,
            symmetry: 0.8,
            ramp_period_s: 1.0,
            rise_time_ns: 800_000_000,
            amplitude_volts: 1.0,
            piezo_scale_factor: 3000.0,
            piezo_settle_time_ms: 2.0,
            ramp_period_us: 1_000_000,
            piezo_settle_time_us: 2000,
            slave_default_offset_v: None,
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    // /// # Panics
    // /// Panics if the RP API returns a catastrophically wrong value
    pub fn apply<'a, 'b>(
        &mut self,
        osc: &mut Oscilloscope,
        ref_ch: Option<&'a mut RawChannel>,
        slave_ch: &'b mut RawChannel,
    ) -> Result<(Option<Channel<'a, Pulse>>, Channel<'b, DC>), ChannelInitializationError> {
        // Create the voltage ramp waveform:
        let steps_up = (16384.0 * self.symmetry) as u16;
        let mut waveform = Vec::<f32>::with_capacity(16384);
        for i in 0..steps_up {
            waveform.push(2.0 * (i as f32) / (steps_up as f32) - 1.0);
        }
        for i in steps_up..16384 {
            waveform.push(
                f32::cos(PI * (i as f32 - steps_up as f32) / (16384 - steps_up) as f32),
            );
        }
        let default_slave_output = (slave_ch.max_output_v() - slave_ch.min_output_v())/2.0;

        osc.set_decimation(self.decimation)?;
        let ref_out = ref_ch.map(|ch| {
            ChannelBuilder::new(ch)
            .with_previous_values()
            .amplitude_v(self.amplitude_volts)
            .period_s(self.ramp_period_s)
            .waveform(waveform)
            .enabled()
            .apply()
        }).transpose()?;
        let slave_out = ChannelBuilder::<DC>::new(slave_ch)
            .with_previous_values()
            .offset_v(
                self.slave_default_offset_v
                    .unwrap_or(default_slave_output)
            )
            .period_s(self.ramp_period_s / 100.0)
            .enabled()
            .apply()?;
        Ok((ref_out, slave_out))
    }

    pub fn slave_default_offset_v(&mut self, offset_v: Option<f32>) -> &mut Self {
        self.slave_default_offset_v = offset_v;
        self
    }

    pub fn piezo_scale_factor(&mut self, scale_factor: f32) -> &mut Self {
        self.piezo_scale_factor = scale_factor;
        self
    }

    pub fn set_decimation(&mut self, decimation: u32) -> &mut Self {
        self.decimation = decimation;
        self.ramp_period_s =
            (16384.0 * (self.decimation as f64) / ADC_SAMPLE_RATE / self.symmetry as f64) as f32;
        self.ramp_period_us = (self.ramp_period_s * 1.0e6) as u64;
        self.rise_time_ns = (self.ramp_period_s * self.symmetry * 1.0e9) as u128;
        self
    }

    #[inline]
    #[must_use]
    pub fn decimation(&self) -> u32 {
        self.decimation
    }

    #[inline]
    #[must_use]
    pub fn rise_time_ns(&self) -> u128 {
        self.rise_time_ns
    }
    #[inline]
    #[must_use]
    pub fn ramp_period_us(&self) -> u64 {
        self.ramp_period_us
    }
    #[inline]
    #[must_use]
    pub fn piezo_settle_time_us(&self) -> u64 {
        self.piezo_settle_time_us
    }

    pub fn set_symmetry(&mut self, symm: f32) -> &mut Self {
        self.symmetry = symm;
        self.ramp_period_s =
            (16384.0 * (self.decimation as f64) / ADC_SAMPLE_RATE / self.symmetry as f64) as f32;
        self.ramp_period_us = (self.ramp_period_s * 1.0e6) as u64;
        self.rise_time_ns = (self.ramp_period_s * self.symmetry * 1.0e9) as u128;
        self
    }
    #[inline]
    #[must_use]
    pub fn symmetry(&self) -> f32 {
        self.symmetry
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn piezo_settle_time_ms(&mut self, time_ms: f32) -> &mut Self {
        self.piezo_settle_time_ms = time_ms;
        self.piezo_settle_time_us = (time_ms * 1000.0) as u64;
        self.ramp_period_s =
            (16384.0 * (self.decimation as f64) / ADC_SAMPLE_RATE / self.symmetry as f64) as f32;
        self.ramp_period_us = (self.ramp_period_s * 1.0e6) as u64;
        self
    }

    pub fn amplitude(&mut self, volts: f32) -> &mut Self {
        self.amplitude_volts = volts;
        self
    }
}
