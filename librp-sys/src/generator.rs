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
use std::marker::PhantomData;
use std::ptr::addr_of_mut;

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
        // Stream,
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

#[macro_export]
macro_rules! ch_switch {
    ($ch:expr, $if_a:expr, $if_b:expr) => {
        match $ch {
            core::Channel::CH_1 => $if_a,
            core::Channel::CH_2 => $if_b,
        }
    };
}

macro_rules! cch {
    ($obj:ident) => {
        $obj.core_ch as core::rp_channel_t
    };
}

#[derive(Debug)]
pub struct RawChannel {
    core_ch: core::RPCoreChannel,
    hardware_offset_v: f32,
    gain_post: f32,
    min_output_v: f32,
    max_output_v: f32,
    _phantom: PhantomData<()>,
}

pub trait ChannelFlavor {}

#[derive(Debug)]
pub struct Base {}
impl ChannelFlavor for Base {}

#[derive(Debug)]
pub struct Pulse {}
impl ChannelFlavor for Pulse {}

#[derive(Debug)]
pub struct DC {}
impl ChannelFlavor for DC {}

/// Nomenclature possibly confusing with Rust's thread-safe ``Channel``. Keeping this way for
/// consistency with the underlying Red Pitaya API.
#[derive(Debug)]
pub struct Channel<'a, Flavor: ChannelFlavor = Base> {
    raw_channel: &'a mut RawChannel,

    ampl_v: f32,
    offset_v: f32,
    waveform_last_value: f32,

    _phantom: PhantomData<Flavor>,
}

#[derive(Debug)]
pub struct Generator {
    pub ch_a: RawChannel,
    pub ch_b: RawChannel,
}
impl Generator {
    #[must_use]
    pub(crate) fn init() -> Self {
        Generator {
            ch_a: RawChannel {
                core_ch: core::RPCoreChannel::CH_1,
                hardware_offset_v: 0.0,
                min_output_v: -1.0,
                max_output_v: 1.0,
                gain_post: 1.0,
                _phantom: PhantomData::<()>,
            },
            ch_b: RawChannel {
                core_ch: core::RPCoreChannel::CH_2,
                hardware_offset_v: 0.0,
                min_output_v: -1.0,
                max_output_v: 1.0,
                gain_post: 1.0,
                _phantom: PhantomData::<()>,
            },
        }
    }

    pub fn reset(&self) -> APIResult<()> {
        wrap_call!(rp_GenReset)
    }
}

impl RawChannel {
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
    pub fn set_amplitude_v(&mut self, amplitude_v: f32) -> APIResult<()> {
        self.set_amplitude_raw(amplitude_v / self.gain_post)
    }
    #[inline]
    pub fn get_amplitude_raw(&mut self) -> APIResult<f32> {
        let mut out: f32 = 0.0;
        wrap_call!(rp_GenGetAmp, cch!(self), addr_of_mut!(out))?;
        Ok(out)
    }
    #[inline]
    pub fn get_amplitude_v(&mut self) -> APIResult<f32> {
        Ok(self.get_amplitude_raw()? * self.gain_post)
    }

    #[inline]
    pub fn set_offset_raw(&mut self, volts: f32) -> APIResult<()> {
        wrap_call!(rp_GenOffset, cch!(self), volts)
    }

    #[inline]
    pub fn set_offset_v(&mut self, offset_v: f32) -> APIResult<()> {
        self.set_offset_raw(offset_v / self.gain_post - self.hardware_offset_v)
    }
    #[inline]
    pub fn get_offset_raw(&mut self) -> APIResult<f32> {
        let mut out: f32 = 0.0;
        wrap_call!(rp_GenGetOffset, cch!(self), addr_of_mut!(out))?;
        Ok(out)
    }
    #[inline]
    pub fn get_offset_v(&mut self) -> APIResult<f32> {
        Ok((self.get_offset_raw()? + self.hardware_offset_v) * self.gain_post)
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
    pub fn set_burst_last_value_raw(&mut self, val_volts: f32) -> APIResult<()> {
        wrap_call!(rp_GenBurstLastValue, cch!(self), val_volts)
    }
    #[inline]
    pub fn set_burst_last_value_v(&mut self, val_volts: f32) -> APIResult<()> {
        self.set_burst_last_value_raw(val_volts / self.gain_post - self.hardware_offset_v)
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
    }
    #[inline]
    pub fn set_hw_offset_v(&mut self, hw_offset_v: f32) {
        self.hardware_offset_v = hw_offset_v;
    }
    #[inline]
    pub fn set_gain_post(&mut self, gain: f32) {
        self.gain_post = gain;
    }
}

impl<'a, Flavor: ChannelFlavor> Channel<'a, Flavor> {
    #[inline]
    pub fn enable(&mut self) -> APIResult<()> {
        self.raw_channel.enable()
    }
    #[inline]
    pub fn disable(&mut self) -> APIResult<()> {
        self.raw_channel.enable()
    }
    #[inline]
    #[must_use]
    pub fn offset(&self) -> f32 {
        self.offset_v
    }
    #[inline]
    #[must_use]
    pub fn amplitude(&self) -> f32 {
        self.ampl_v
    }
    #[inline]
    pub fn set_amplitude(&mut self, ampl_v: f32) -> APIResult<()> {
        self.ampl_v = ampl_v.clamp(
            0.0,
            (self.raw_channel.max_output_v - self.raw_channel.min_output_v) / 2.0,
        );
        self.offset_v = self.offset_v.clamp(
            self.raw_channel.min_output_v + self.ampl_v.abs(),
            self.raw_channel.max_output_v - self.ampl_v.abs(),
        );
        self.raw_channel.set_offset_v(self.offset_v)?;
        self.raw_channel
            .set_burst_last_value_v(self.offset_v + self.ampl_v * self.waveform_last_value)
    }
    #[inline]
    pub fn set_offset(&mut self, offset_v: f32) -> APIResult<()> {
        self.offset_v = offset_v.clamp(
            self.raw_channel.min_output_v + self.ampl_v.abs(),
            self.raw_channel.max_output_v - self.ampl_v.abs(),
        );
        self.raw_channel.set_offset_v(self.offset_v)?;
        self.raw_channel
            .set_burst_last_value_v(self.offset_v + self.ampl_v * self.waveform_last_value)
    }
    #[inline]
    pub fn adjust_offset(&mut self, adjustment_v: f32) -> APIResult<()> {
        self.set_offset(adjustment_v + self.offset_v)
    }
}

#[derive(Debug)]
pub enum ChannelInitializationError {
    NaNValue,          // one of the given parameters is NaN
    InfiniteValue,     // The gain or hardware offset specified is infinite
    ZeroGain,          // the specified gain parameter is zero
    RangeInversion,    // max voltage less than min voltage
    RangeInaccessible, // the specified range is not accessible given the gain and hardware offset
    NoWaveform,        // Attempted to initialize a PulseChannel without providing a pulse waveform

    PitayaAPIError(APIError), // Error in writing API commands
}
impl From<APIError> for ChannelInitializationError {
    fn from(e: APIError) -> Self {
        Self::PitayaAPIError(e)
    }
}

//TODO: consider swapping `Option<Vec>` for `Option<&mut [f32]>`
#[derive(Debug)]
pub struct ChannelBuilder<'a, Flavor: ChannelFlavor = Base> {
    ch: &'a mut RawChannel,
    hardware_offset_v: f32,
    gain_post: f32,
    range_v: std::ops::Range<f32>,

    freq_hz: f32,
    offset_v: f32,
    amplitude_v: f32,
    waveform: Option<Vec<f32>>,

    enable: bool,

    phantom: PhantomData<Flavor>,
}
impl<'a, Flavor: ChannelFlavor> ChannelBuilder<'a, Flavor> {
    #[must_use]
    pub fn new(channel: &'a mut RawChannel) -> Self {
        ChannelBuilder {
            ch: channel,
            hardware_offset_v: 0.0,
            gain_post: 1.0,
            range_v: -1.0..1.0,
            freq_hz: 1000.0,
            offset_v: 0.0,
            amplitude_v: 1.0,
            waveform: None,
            enable: true,
            phantom: PhantomData::<Flavor>,
        }
    }
    #[must_use]
    pub fn with_previous_values(mut self) -> Self {
        self.hardware_offset_v = self.ch.hardware_offset_v;
        self.gain_post = self.ch.gain_post;
        self.range_v = (self.ch.min_output_v)..(self.ch.max_output_v);
        self.amplitude_v = self.ch.get_amplitude_v().unwrap();
        self.offset_v = self.ch.get_offset_v().unwrap();
        self
    }
    fn apply_base(&mut self) -> Result<(), ChannelInitializationError> {
        if self.hardware_offset_v.is_nan()
            || self.gain_post.is_nan()
            || self.range_v.start.is_nan()
            || self.range_v.end.is_nan()
        {
            return Err(ChannelInitializationError::NaNValue);
        }
        if self.gain_post.is_infinite() || self.hardware_offset_v.is_infinite() {
            return Err(ChannelInitializationError::InfiniteValue);
        }
        if self.gain_post == 0.0 {
            return Err(ChannelInitializationError::ZeroGain);
        }
        if self.range_v.is_empty() {
            return Err(ChannelInitializationError::RangeInversion);
        }
        self.clamp_range();
        if self.range_v.is_empty() {
            return Err(ChannelInitializationError::RangeInaccessible);
        }
        self.ch.disable()?;
        self.ch.hardware_offset_v = self.hardware_offset_v;
        self.ch.gain_post = self.gain_post;
        self.ch.min_output_v = self.range_v.start;
        self.ch.max_output_v = self.range_v.end;
        self.amplitude_v = self.amplitude_v.clamp(
            // (self.range_v.start - self.range_v.end)/2.0,
            0.0,
            (self.range_v.end - self.range_v.start) / 2.0,
        );
        self.offset_v = self.offset_v.clamp(
            self.range_v.start + self.amplitude_v.abs(),
            self.range_v.end - self.amplitude_v.abs(),
        );

        self.ch.set_freq(self.freq_hz)?;
        self.ch.set_offset_v(self.offset_v)?;
        self.ch.set_amplitude_v(self.amplitude_v)?;

        Ok(())
    }

    fn clamp_range(&mut self) {
        // Reduce the bounds limitation parameters to the values accessible by the Red Pitaya
        // hardware. If both the specified bounds are, say, less than the minimum value accessible
        // by the pitaya hardware, then this new range will be empty.
        let neg_out = self.gain_post * (-1.0 + self.hardware_offset_v);
        let pos_out = self.gain_post * (1.0 + self.hardware_offset_v);
        let lower = neg_out.min(pos_out);
        let upper = neg_out.max(pos_out);
        self.range_v =
            (self.range_v.start.clamp(lower, upper))..self.range_v.end.clamp(lower, upper);
    }

    #[must_use]
    pub fn hardware_offset(mut self, volts: f32) -> Self {
        self.hardware_offset_v = volts;
        self
    }
    #[must_use]
    pub fn gain_post(mut self, gain: f32) -> Self {
        self.gain_post = gain;
        self
    }
    #[must_use]
    pub fn output_range(mut self, lower: f32, upper: f32) -> Self {
        self.range_v.start = lower;
        self.range_v.end = upper;
        self
    }
    #[must_use]
    pub fn offset_v(mut self, offset_v: f32) -> Self {
        self.offset_v = offset_v;
        self
    }
    #[must_use]
    pub fn freq_hz(mut self, freq_hz: f32) -> Self {
        self.freq_hz = freq_hz;
        self
    }
    #[must_use]
    pub fn period_s(mut self, period_s: f32) -> Self {
        self.freq_hz = 1.0 / period_s;
        self
    }
    #[must_use]
    pub fn enabled(mut self) -> Self {
        self.enable = true;
        self
    }
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enable = false;
        self
    }
    #[must_use]
    pub fn enable(mut self, onoff: bool) -> Self {
        self.enable = onoff;
        self
    }
}

impl<'a> ChannelBuilder<'a, Pulse> {
    #[must_use]
    pub fn waveform(mut self, wav: Vec<f32>) -> Self {
        self.waveform = Some(wav);
        self
    }
    #[must_use]
    pub fn amplitude_v(mut self, amplitude_v: f32) -> Self {
        self.amplitude_v = amplitude_v;
        self
    }
    pub fn apply(mut self) -> Result<Channel<'a, Pulse>, ChannelInitializationError> {
        self.apply_base()?;
        let mut wav = self
            .waveform
            .ok_or(ChannelInitializationError::NoWaveform)?;
        let last_value = wav.pop().ok_or(ChannelInitializationError::NoWaveform)?;
        wav.push(last_value);

        self.ch
            .set_trigger_source(GenTriggerSource::ExternalRisingEdge)?;

        self.ch.set_waveform_type(WaveformType::Arbitrary)?;
        self.ch.set_arb_waveform(&mut wav)?;
        self.ch.set_mode(GenMode::Burst)?;
        self.ch.set_burst_count(1)?;
        self.ch.set_burst_repetitions(1)?;
        self.ch
            .set_burst_last_value_v(self.amplitude_v * last_value + self.offset_v)?;
        if self.enable {
            self.ch.enable()?;
        }
        Ok(Channel {
            raw_channel: self.ch,
            ampl_v: self.amplitude_v,
            offset_v: self.offset_v,
            waveform_last_value: last_value,
            _phantom: PhantomData::<Pulse>,
        })
    }
}

impl<'a> ChannelBuilder<'a, DC> {
    pub fn apply(mut self) -> Result<Channel<'a, DC>, ChannelInitializationError> {
        self.amplitude_v = 0.0;
        self.apply_base()?;
        let mut wav = vec![0f32; 8];
        self.ch
            .set_trigger_source(GenTriggerSource::ExternalRisingEdge)?;
        self.ch.set_waveform_type(WaveformType::Arbitrary)?;
        self.ch.set_arb_waveform(&mut wav)?;
        self.ch.set_mode(GenMode::Burst)?;
        self.ch.set_burst_count(1)?;
        self.ch.set_burst_repetitions(1)?;
        self.ch.set_burst_last_value_v(self.offset_v)?;
        if self.enable {
            self.ch.enable()?;
        }
        Ok(Channel {
            raw_channel: self.ch,
            ampl_v: self.amplitude_v,
            offset_v: self.offset_v,
            waveform_last_value: 0.0,
            _phantom: PhantomData::<DC>,
        })
    }
}
impl<'a> ChannelBuilder<'a, Base> {
    pub fn apply(mut self) -> Result<Channel<'a, Base>, ChannelInitializationError> {
        self.apply_base()?;
        if self.enable {
            self.ch.enable()?;
        }
        Ok(Channel {
            raw_channel: self.ch,
            ampl_v: self.amplitude_v,
            offset_v: self.offset_v,
            waveform_last_value: 0.0,
            _phantom: PhantomData::<Base>,
        })
    }
}
