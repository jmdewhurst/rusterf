#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]
#![allow(clippy::module_name_repetitions)]
use std::f32::consts::PI;

use librp_sys::core::{APIResult, ADC_SAMPLE_RATE};
use librp_sys::generator::{DCChannel, PulseChannel};
use librp_sys::oscilloscope::Oscilloscope;
use serde::{Deserialize, Serialize};

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
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn apply(
        &mut self,
        osc: &mut Oscilloscope,
        ref_ch: Option<&mut PulseChannel>,
        slave_ch: &mut DCChannel,
    ) -> APIResult<()> {
        // Create the voltage ramp waveform:
        let steps_up = (16384.0 * self.symmetry) as u16;
        let mut waveform = Vec::<f32>::with_capacity(16384);
        for i in 0..steps_up {
            waveform.push(-0.5 * (i as f32) / (steps_up as f32));
        }
        for i in steps_up..16384 {
            waveform.push(f32::cos(PI * (i as f32) / (16384 - steps_up) as f32) / 2.0);
        }

        osc.set_decimation(self.decimation)?;
        if let Some(ref_ch) = ref_ch {
            ref_ch.set_period(self.ramp_period_s)?;
            ref_ch.set_amplitude(self.amplitude_volts)?;
            ref_ch.set_waveform(&mut waveform);
            ref_ch.enable()?;
        }
        slave_ch.set_period(self.ramp_period_s / 2.0)?;

        Ok(())
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
        self.rise_time_ns = (self.ramp_period_s * self.symmetry * 1e-9) as u128;
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
        self.rise_time_ns = (self.ramp_period_s * self.symmetry * 1e-9) as u128;
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

impl Default for DaqSetup {
    fn default() -> Self {
        DaqSetup::new()
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct DaqSerialize {
    amplitude_volts: f32,
    piezo_scale_factor: f32, // units of nm / Volt
    piezo_settle_time_ms: f32,
}

impl DaqSerialize {
    fn into_ramp(self) -> DaqSetup {
        let mut out = DaqSetup::new();
        out.amplitude(self.amplitude_volts);
        out.piezo_settle_time_ms(self.piezo_settle_time_ms);
        out
    }

    fn from_ramp(ramp: &DaqSetup) -> Self {
        DaqSerialize {
            amplitude_volts: ramp.amplitude_volts,
            piezo_scale_factor: ramp.piezo_scale_factor,
            piezo_settle_time_ms: ramp.piezo_settle_time_ms,
        }
    }
}

impl<'de> Deserialize<'de> for DaqSetup {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(DaqSerialize::deserialize(d)?.into_ramp())
    }
}

impl Serialize for DaqSetup {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        DaqSerialize::from_ramp(self).serialize(serializer)
    }
}
