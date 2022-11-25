#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
use crate::core::{APIError, APIError::RP_OK, APIResult};
use crate::{core, pitaya};
use enum_primitive::*;
use std::ffi::c_int;
use std::marker::PhantomData;
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
        Internal = 0,
        ExternalRisingEdge, // External trigger is on DIO0_P; this is not configurable
        ExternalFallingEdge,
}
}

pub struct Channel<'a> {
    core_ch: core::Channel,
    phantom: PhantomData<&'a Generator<'a>>,
}

pub struct Generator<'a> {
    pub ch_a: Channel<'a>,
    pub ch_b: Channel<'a>,
    phantom: PhantomData<&'a pitaya::Pitaya>,
}

impl<'a> Channel<'a> {
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn enable(&mut self) -> APIResult<()> {
        match APIError::from_i32(unsafe {
            core::rp_GenOutEnable(self.core_ch as core::rp_channel_t)
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn disable(&mut self) -> APIResult<()> {
        match APIError::from_i32(unsafe {
            core::rp_GenOutDisable(self.core_ch as core::rp_channel_t)
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_amplitude(&mut self, volts: f32) -> APIResult<()> {
        // sets the arb. waveform gen. amplitude in volts; range 0 -- 1
        match APIError::from_i32(unsafe {
            core::rp_GenAmp(self.core_ch as core::rp_channel_t, volts)
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_offset(&mut self, volts: f32) -> APIResult<()> {
        // sets the arb. waveform gen. offset in volts; range -1 -- 1 (I think)
        match APIError::from_i32(unsafe {
            core::rp_GenOffset(self.core_ch as core::rp_channel_t, volts)
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_freq(&mut self, freq_hz: f32) -> APIResult<()> {
        // sets the arb. waveform gen. frequency in Hz.
        match APIError::from_i32(unsafe {
            core::rp_GenFreq(self.core_ch as core::rp_channel_t, freq_hz)
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_period(&mut self, period_s: f32) -> APIResult<()> {
        // Helper function for ergonomics; equivalent to set_freq.
        // Sets the arb. waveform gen. period in seconds.
        match APIError::from_i32(unsafe {
            core::rp_GenFreq(self.core_ch as core::rp_channel_t, 1.0 / period_s)
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_waveform_type(&mut self, wav_type: WaveformType) -> APIResult<()> {
        // Set the AWG to one of the predefined waveforms
        match APIError::from_i32(unsafe {
            core::rp_GenWaveform(
                self.core_ch as core::rp_channel_t,
                wav_type as core::rp_waveform_t,
            )
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }

    /// Set the AWG into arbitrary waveform mode, and set its waveform to the given vector,
    /// which should take values in [-1.0, 1.0]. The api should be stable (i.e. not crash) for
    /// waveform vectors of arbitrary length, but its behavior is unknown (to me) unless waveform
    /// is of length exactly 16384.
    /// Should not be passed a vector of length > ``u32::max_value()`` --- why would you even
    /// think to do that?
    /// TODO: I'm not sure if a mutable reference to a slice is the right data type --- consider
    /// switching to an owned vector?
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_arb_waveform(&mut self, waveform: &mut [f32]) -> APIResult<()> {
        self.set_waveform_type(WaveformType::Arbitrary)?;

        match APIError::from_i32(unsafe {
            core::rp_GenArbWaveform(
                self.core_ch as core::rp_channel_t,
                waveform.as_mut_ptr(),
                waveform.len() as u32,
            )
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_mode(&mut self, mode: GenMode) -> APIResult<()> {
        // Set the AWG to the given mode
        match APIError::from_i32(unsafe {
            core::rp_GenMode(
                self.core_ch as core::rp_channel_t,
                mode as core::rp_gen_mode_t,
            )
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_burst_count(&mut self, count: i32) -> APIResult<()> {
        // Set how many instances of the waveform the burst mode fires when triggered.
        match APIError::from_i32(unsafe {
            core::rp_GenBurstCount(self.core_ch as core::rp_channel_t, count as c_int)
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_burst_last_value(&mut self, val_volts: f32) -> APIResult<()> {
        // By default, when the AWG fires a burst of waveforms, it then sets the voltage to 0
        // after the end of the final waveform. This function sets the voltage the AWG outputs
        // after finishing the burst.
        match APIError::from_i32(unsafe {
            core::rp_GenBurstLastValue(self.core_ch as core::rp_channel_t, val_volts)
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_trigger_source(&mut self, source: GenTriggerSource) -> APIResult<()> {
        // Sets the source of the trigger for the AWG. Internal is triggered directly from
        // software; external are on DIO0_P.
        match APIError::from_i32(unsafe {
            core::rp_GenTriggerSource(
                self.core_ch as core::rp_channel_t,
                source as core::rp_trig_src_t,
            )
        })
        .unwrap()
        {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_single_pulse_mode(&mut self, waveform: &mut [f32]) -> APIResult<()> {
        // Helper function that sets the given waveform, puts the AWG in arbitrary mode, sets
        // burst mode, and sets the burst count to 1.
        self.set_arb_waveform(waveform)?;
        self.set_mode(GenMode::Burst)?;
        self.set_burst_count(1)?;
        Ok(())
    }
}

impl<'a> Generator<'a> {
    #[must_use]
    pub fn init(_: &'a pitaya::Pitaya) -> Self {
        Generator {
            ch_a: Channel {
                core_ch: core::Channel::CH_1,
                phantom: PhantomData,
            },
            ch_b: Channel {
                core_ch: core::Channel::CH_2,
                phantom: PhantomData,
            },
            phantom: PhantomData,
        }
    }
}

pub struct PulseChannel<'a> {
    ch: &'a mut Channel<'a>,
    _waveform: Vec<f32>,
    waveform_last_value: f32,
    offset_volt: f32,
    amplitude_volt: f32,
}

impl<'a> PulseChannel<'a> {
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn init(
        ch: &'a mut Channel<'a>,
        mut waveform: Vec<f32>,
        ampl_volts: f32,
    ) -> APIResult<Self> {
        let last_value = waveform[waveform.len() - 1];
        ch.set_single_pulse_mode(&mut waveform as &mut [f32])?;
        ch.set_offset(0.)?;
        ch.set_burst_last_value(last_value * ampl_volts)?;
        Ok(PulseChannel {
            ch,
            _waveform: waveform,
            waveform_last_value: last_value,
            offset_volt: 0.,
            amplitude_volt: ampl_volts,
        })
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn enable(&mut self) -> APIResult<()> {
        self.ch.enable()
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn disable(&mut self) -> APIResult<()> {
        self.ch.disable()
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_trigger_source(&mut self, source: GenTriggerSource) -> APIResult<()> {
        self.ch.set_trigger_source(source)
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_amplitude(&mut self, volts: f32) -> APIResult<()> {
        self.ch.set_amplitude(volts)?;
        self.amplitude_volt = volts;
        self.ch.set_burst_last_value(
            (self.amplitude_volt * self.waveform_last_value) + self.offset_volt,
        )?;
        Ok(())
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_offset(&mut self, volts: f32) -> APIResult<()> {
        self.ch.set_offset(volts)?;
        self.offset_volt = volts;
        self.ch.set_burst_last_value(
            (self.amplitude_volt * self.waveform_last_value) + self.offset_volt,
        )?;
        Ok(())
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_period(&mut self, period_s: f32) -> APIResult<()> {
        self.ch.set_period(period_s)
    }
}
