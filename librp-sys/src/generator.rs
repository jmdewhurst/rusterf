#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
use crate::core;
use crate::core::{APIError, APIError::RP_OK, APIResult};
use enum_primitive::*;
use std::ffi::c_int;
use std::thread;
use std::time::Duration;
// use std::mem::MaybeUninit;

enum_from_primitive! {
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum WaveformType {
        Sine = 0,
        Square,
        Triangle,
        RampUp,
        RampDown,
        DC,
        PWM,
        Arbitrary,
        DCNeg,
        Sweep,
}
}

enum_from_primitive! {
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum GenMode {
        Continuous = 0,
        Burst,
        Stream,
}
}

enum_from_primitive! {
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum GenTriggerSource {
        Internal = 1,
        ExternalRisingEdge, // External trigger is on DIO0_P; this is not configurable
        ExternalFallingEdge,
}
}

/// Nomenclature possibly confusing with Rust's thread-safe ``Channel``. Keeping this way for
/// consistency with the underlying Red Pitaya API.
#[derive(Debug)]
pub struct Channel {
    core_ch: core::Channel,
    ampl_v: f32,
    offset_v: f32,
    hardware_offset_v: f32,
    gain_post: f32,
    min_output_v: f32,
    max_output_v: f32,
}

#[derive(Debug)]
pub struct Generator {
    pub ch_a: Channel,
    pub ch_b: Channel,
}

#[derive(Debug)]
pub struct PulseChannel<'a> {
    pub ch: &'a mut Channel,
    waveform_last_value: f32,
}

#[derive(Debug)]
pub struct DCChannel<'a> {
    pub ch: &'a mut Channel,
}

macro_rules! cch {
    ($obj:ident) => {
        $obj.core_ch as core::rp_channel_t
    };
}

/// # Errors
/// If an RP API call returns a failure code, this returns Err containing the failure.
/// # Panics
/// Panics if the RP API returns a catastrophically wrong value
impl Channel {
    #[inline]
    pub fn enable(&mut self) -> APIResult<()> {
        wrap_call!(rp_GenOutEnable, cch!(self))
    }
    #[inline]
    pub fn disable(&mut self) -> APIResult<()> {
        wrap_call!(rp_GenOutDisable, cch!(self))
    }

    #[inline]
    pub fn set_amplitude_raw(&mut self, volts: f32) -> APIResult<()> {
        wrap_call!(rp_GenAmp, cch!(self), volts)
    }

    #[inline]
    pub fn set_offset_raw(&mut self, volts: f32) -> APIResult<()> {
        wrap_call!(rp_GenOffset, cch!(self), volts)
    }

    #[inline]
    pub fn set_freq(&mut self, freq_hz: f32) -> APIResult<()> {
        wrap_call!(rp_GenFreq, cch!(self), freq_hz)
    }

    #[inline]
    pub fn set_period(&mut self, period_s: f32) -> APIResult<()> {
        wrap_call!(rp_GenFreq, cch!(self), 1.0 / period_s)
    }

    #[inline]
    pub fn set_waveform_type(&mut self, wav_type: WaveformType) -> APIResult<()> {
        wrap_call!(rp_GenWaveform, cch!(self), wav_type as core::rp_waveform_t)
    }

    /// Set the AWG into arbitrary waveform mode, and set its waveform to the given vector,
    /// which should take values in [-1.0, 1.0]. The api should be stable (i.e. not crash) for
    /// waveform vectors of arbitrary length, but its behavior is unknown (to me) unless waveform
    /// is of length exactly 16384.
    /// Should not be passed a vector of length > ``u32::max_value()`` --- why would you even
    /// think to do that?
    #[inline]
    pub fn set_arb_waveform(&mut self, waveform: &mut [f32]) -> APIResult<()> {
        self.set_waveform_type(WaveformType::Arbitrary)?;
        wrap_call!(
            rp_GenArbWaveform,
            cch!(self),
            waveform.as_mut_ptr(),
            waveform.len() as u32
        )
    }

    #[inline]
    pub fn set_mode(&mut self, mode: GenMode) -> APIResult<()> {
        wrap_call!(rp_GenMode, cch!(self), mode as core::rp_gen_mode_t)
    }

    #[inline]
    pub fn set_burst_count(&mut self, count: i32) -> APIResult<()> {
        wrap_call!(rp_GenBurstCount, cch!(self), count as c_int)
    }

    #[inline]
    pub fn set_burst_repetitions(&mut self, repetitions: i32) -> APIResult<()> {
        wrap_call!(rp_GenBurstRepetitions, cch!(self), repetitions as c_int)
    }

    /// By default, when the AWG fires a burst of waveforms, it then sets the voltage to 0
    /// after the end of the final waveform. This function sets the voltage the AWG outputs
    /// after finishing the burst.
    #[inline]
    pub fn set_burst_last_value(&mut self, val_volts: f32) -> APIResult<()> {
        wrap_call!(
            rp_GenBurstLastValue,
            cch!(self),
            val_volts / self.gain_post - self.hardware_offset_v
        )
    }

    /// Sets the source of the trigger for the AWG. Internal is triggered directly from
    /// software; external are on `DIO0_P`.
    #[inline]
    pub fn set_trigger_source(&mut self, source: GenTriggerSource) -> APIResult<()> {
        wrap_call!(
            rp_GenTriggerSource,
            cch!(self),
            source as core::rp_trig_src_t
        )
    }

    #[must_use]
    #[inline]
    pub fn max_output_v(&self) -> f32 {
        self.max_output_v
    }
    #[must_use]
    #[inline]
    pub fn min_output_v(&self) -> f32 {
        self.min_output_v
    }

    #[inline]
    pub fn set_output_range(&mut self, min_v: f32, max_v: f32) {
        //TODO: guanrantee `min_v < max_v` and neither is NaN
        self.min_output_v = min_v;
        self.max_output_v = max_v;
        let _ = self.set_amplitude_v(self.ampl_v);
    }
    #[inline]
    pub fn set_hw_offset_v(&mut self, hw_offset_v: f32) {
        self.hardware_offset_v = hw_offset_v;
        let _ = self.set_amplitude_v(self.ampl_v);
    }
    #[inline]
    pub fn set_gain_post(&mut self, gain: f32) {
        self.gain_post = gain;
        let _ = self.set_amplitude_v(self.ampl_v);
    }

    /// # Errors:
    /// If setting the amplitude would cause the function generator to exceed the user-configured
    /// voltage range, it will set that amplitude, clamp the offset, and return `Err` containing
    /// the new offset.
    #[inline]
    #[allow(clippy::float_cmp)]
    pub fn set_amplitude_v(&mut self, ampl_v: f32) -> Result<(), f32> {
        self.ampl_v = ampl_v.clamp(0.0, 1.0 * self.gain_post);
        let _ = self.set_amplitude_raw(ampl_v / self.gain_post);
        let old_val = self.offset_v;
        let new_val = self.set_offset_v(old_val);
        if new_val != old_val {
            return Err(new_val);
        }
        Ok(())
    }
    /// sets the offset, clamped to within the user-configured output range (including amplitude).
    /// Returns the set offset voltage.
    #[inline]
    pub fn set_offset_v(&mut self, offset_v: f32) -> f32 {
        // Calling this function with `offset_v == 0.0` should set the 'zero point' of the waveform
        // to halfway between the minimum and maximum allowed values
        self.offset_v = offset_v.clamp(
            self.min_output_v + self.ampl_v,
            self.max_output_v - self.ampl_v,
        );
        self.set_offset_raw(self.offset_v / self.gain_post - self.hardware_offset_v)
            .expect("RP API calls shouldn't fail");
        self.offset_v
    }
}

impl Generator {
    #[must_use]
    pub(crate) fn init() -> Self {
        Generator {
            ch_a: Channel {
                core_ch: core::Channel::CH_1,
                ampl_v: 1.0,
                offset_v: 0.0,
                hardware_offset_v: 0.0,
                min_output_v: -1.0,
                max_output_v: 1.0,
                gain_post: 1.0,
            },
            ch_b: Channel {
                ampl_v: 1.0,
                offset_v: 0.0,
                core_ch: core::Channel::CH_2,
                hardware_offset_v: 0.0,
                min_output_v: -1.0,
                max_output_v: 1.0,
                gain_post: 1.0,
            },
        }
    }

    pub fn reset(&self) -> APIResult<()> {
        wrap_call!(rp_GenReset)
    }
}

impl<'a> DCChannel<'a> {
    pub fn init(ch: &'a mut Channel) -> APIResult<Self> {
        ch.set_waveform_type(WaveformType::RampUp)?;
        let _ = ch.set_amplitude_v(0.0);
        ch.set_mode(GenMode::Burst)?;
        ch.set_burst_count(1)?;
        ch.set_burst_repetitions(1)?;
        ch.set_offset_v((ch.max_output_v - ch.min_output_v) / 2.0);
        ch.set_burst_last_value(ch.offset_v)?;
        ch.enable()?;
        Ok(DCChannel { ch })
    }
    #[inline]
    pub fn enable(&mut self) -> APIResult<()> {
        self.ch.enable()
    }
    #[inline]
    pub fn disable(&mut self) -> APIResult<()> {
        self.ch.disable()
    }
    #[inline]
    pub fn set_offset(&mut self, offset_v: f32) {
        self.ch.set_offset_v(offset_v);
        let _ = self.ch.set_burst_last_value(offset_v);
    }
    #[inline]
    #[must_use]
    pub fn offset_v(&self) -> f32 {
        self.ch.offset_v
    }
    #[inline]
    pub fn set_period(&mut self, period_s: f32) -> APIResult<()> {
        self.ch.set_period(period_s)
    }
    #[inline]
    pub fn increment_offset(&mut self, volts: f32) {
        self.set_offset(volts + self.ch.offset_v);
    }
}

impl<'a> PulseChannel<'a> {
    pub fn init(ch: &'a mut Channel, mut waveform: Vec<f32>, ampl_volts: f32) -> APIResult<Self> {
        let last_value = waveform[waveform.len() - 1];
        ch.set_arb_waveform(&mut waveform)?;
        ch.set_mode(GenMode::Burst)?;
        ch.set_burst_count(1)?;
        ch.set_burst_repetitions(1)?;
        ch.set_offset_v((ch.max_output_v - ch.min_output_v) / 2.0);
        ch.set_burst_last_value((ch.ampl_v * last_value) + ch.offset_v);
        let _ = ch.set_amplitude_v(ampl_volts);
        ch.set_trigger_source(GenTriggerSource::ExternalRisingEdge);
        Ok(PulseChannel {
            ch,
            waveform_last_value: last_value,
        })
    }

    #[inline]
    pub fn enable(&mut self) -> APIResult<()> {
        self.ch.enable()
    }
    #[inline]
    pub fn disable(&mut self) -> APIResult<()> {
        self.ch.disable()
    }

    #[inline]
    #[must_use]
    pub fn amplitude_v(&self) -> f32 {
        self.ch.ampl_v
    }
    #[inline]
    #[must_use]
    pub fn offset_v(&self) -> f32 {
        self.ch.offset_v
    }

    /// Disables the channel in question, then sets the given waveform. IMPORTANT: in order to use
    /// the channel after this, you must call `.enable()` on it.
    pub fn set_waveform(&mut self, waveform: &mut [f32]) -> APIResult<()> {
        // self.ch.disable()?;
        self.ch.set_arb_waveform(waveform)?;
        thread::sleep(Duration::from_millis(50));
        self.waveform_last_value = waveform[waveform.len() - 1];
        self.set_amplitude(self.ch.ampl_v);
        Ok(())
    }
    #[inline]
    pub fn set_trigger_source(&mut self, source: GenTriggerSource) -> APIResult<()> {
        self.ch.set_trigger_source(source)
    }
    #[inline]
    pub fn set_amplitude(&mut self, volts: f32) -> APIResult<()> {
        let _ = self.ch.set_amplitude_v(volts);
        self.set_offset(self.ch.offset_v)
    }
    #[inline]
    pub fn set_offset(&mut self, volts: f32) -> APIResult<()> {
        let volts_checked = self.ch.set_offset_v(volts);
        self.ch
            .set_burst_last_value((self.ch.ampl_v * self.waveform_last_value) + volts_checked)
    }
    #[inline]
    pub fn increment_offset(&mut self, volts: f32) -> APIResult<()> {
        self.set_offset(volts + self.ch.offset_v)
    }
    #[inline]
    pub fn set_period(&mut self, period_s: f32) -> APIResult<()> {
        self.ch.set_period(period_s)
    }
}
