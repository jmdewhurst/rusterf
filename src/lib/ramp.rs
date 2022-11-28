#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

#[derive(Debug)]
pub struct Ramp {
    pub amplitude_volts: f32,
    pub preamp_gain: f32,
    amplitude_raw: f32,
    pub piezo_scale_factor: f32, // units of nm / Volt
    pub piezo_settle_time_ms: f32,
    piezo_settle_time_us: i64,
}

impl Ramp {
    pub fn new() -> Self {
        Ramp {
            amplitude_volts: 1.0,
            preamp_gain: 1.0,
            amplitude_raw: 1.0,
            piezo_scale_factor: 3000.0,
            piezo_settle_time_ms: 2.0,
            piezo_settle_time_us: 2000,
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn set_piezo_settle_time(&mut self, time_ms: f32) {
        self.piezo_settle_time_ms = time_ms;
        self.piezo_settle_time_us = (time_ms * 1000.0) as i64;
    }

    pub fn set_amplitude(&mut self, volts: f32) {
        self.amplitude_volts = volts;
        self.amplitude_raw = volts / self.preamp_gain;
    }

    pub fn set_preamp_gain(&mut self, gain: f32) {
        self.preamp_gain = gain;
        self.amplitude_raw = self.amplitude_volts / self.preamp_gain;
    }
}
