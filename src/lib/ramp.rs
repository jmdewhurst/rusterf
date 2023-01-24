#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]
#![allow(clippy::module_name_repetitions)]
use std::f32::consts::PI;

use librp_sys::core::{APIResult, ADC_SAMPLE_RATE};
use librp_sys::generator;
use librp_sys::oscilloscope::Oscilloscope;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct DaqSetup {
    pub decimation: u32,
    pub symmetry: f32,
    pub ramp_period: f32,
    pub amplitude_volts: f32,
    pub preamp_gain: f32,
    amplitude_raw: f32,
    pub piezo_scale_factor: f32, // units of nm / Volt
    pub piezo_settle_time_ms: f32,
    piezo_settle_time_us: i64,
}

impl DaqSetup {
    pub fn new() -> Self {
        DaqSetup {
            decimation: 1,
            symmetry: 0.8,
            ramp_period: 1.0,
            amplitude_volts: 1.0,
            preamp_gain: 1.0,
            amplitude_raw: 1.0,
            piezo_scale_factor: 3000.0,
            piezo_settle_time_ms: 2.0,
            piezo_settle_time_us: 2000,
        }
    }

    pub fn apply(
        &mut self,
        osc: &mut Oscilloscope,
        gen: &mut generator::PulseChannel,
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
        gen.set_amplitude(self.amplitude_raw)?;
        gen.set_period(self.ramp_period)?;
        Ok(())
    }

    pub fn piezo_scale_factor(&mut self, scale_factor: f32) -> &mut Self {
        self.piezo_scale_factor = scale_factor;
        self
    }

    pub fn decimation(&mut self, decimation: u32) -> &mut Self {
        self.decimation = decimation;
        self.ramp_period =
            (16384.0 / ADC_SAMPLE_RATE / (self.decimation as f64) / self.symmetry as f64) as f32;
        self
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn piezo_settle_time(&mut self, time_ms: f32) -> &mut Self {
        self.piezo_settle_time_ms = time_ms;
        self.piezo_settle_time_us = (time_ms * 1000.0) as i64;
        self.ramp_period =
            (16384.0 / ADC_SAMPLE_RATE / (self.decimation as f64) / self.symmetry as f64) as f32;
        self
    }

    pub fn amplitude(&mut self, volts: f32) -> &mut Self {
        self.amplitude_volts = volts;
        self.amplitude_raw = volts / self.preamp_gain;
        self
    }

    pub fn preamp_gain(&mut self, gain: f32) -> &mut Self {
        self.preamp_gain = gain;
        self.amplitude_raw = self.amplitude_volts / self.preamp_gain;
        self
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct DaqSerialize {
    amplitude_volts: f32,
    preamp_gain: f32,
    piezo_scale_factor: f32, // units of nm / Volt
    piezo_settle_time_ms: f32,
}

impl DaqSerialize {
    fn into_ramp(self) -> DaqSetup {
        let mut out = DaqSetup::new();
        out.amplitude(self.amplitude_volts);
        out.piezo_settle_time(self.piezo_settle_time_ms);
        out.preamp_gain(self.preamp_gain);
        out
    }

    fn from_ramp(ramp: &DaqSetup) -> Self {
        DaqSerialize {
            amplitude_volts: ramp.amplitude_volts,
            preamp_gain: ramp.preamp_gain,
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
