#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::unused_self)]

use crate::core::{APIError, APIError::RP_OK, APIResult, Channel};
use crate::{core, pitaya, resources};
use enum_primitive::*;
use std::mem::MaybeUninit;
use std::ptr::read_volatile;

// Red pitaya samples at 125 MHz
pub const BASE_SAMPLE_RATE: f32 = 125_000_000.0;

enum_from_primitive! {
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum TrigState {
        Triggered = 0, // means triggered or disabled
        Waiting, // Waiting means ARMED
}
}

enum_from_primitive! {
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub enum TrigSrc {
        Disabled = 0,
        Now,
        ChARising,
        ChAFalling,
        ChBRising,
        ChBFalling,
        ExtRising,
        ExtFalling,
        GenRising,
        GenFalling,
}
}

#[derive(Debug)]
pub struct ScopeRegion {
    skip_start: u32,
    skip_end: u32,
    skip_rate: u32,
    num_points: usize,
}

#[derive(Debug)]
pub struct Oscilloscope<'a> {
    chA_buff_raw: *const u32,
    chB_buff_raw: *const u32,
    // maintains arrays of recent scope data, culled to ``region`` and converted to floating pt
    pub chA_buff_float: Vec<f32>,
    pub chB_buff_float: Vec<f32>,
    // arrays of a FULL waveform, as the raw u32, for caching a waveform to send over a socket
    pub chA_last_waveform: Vec<u32>,
    pub chB_last_waveform: Vec<u32>,
    region: ScopeRegion,
    _resource: &'a mut resources::ScopeResource,
}

impl<'a> Oscilloscope<'a> {
    #[must_use]
    pub fn init(pit: &'a mut pitaya::Pitaya) -> Self {
        Oscilloscope {
            chA_buff_raw: unsafe { core::rp_jmd_AcqGetRawBuffer(0) },
            chB_buff_raw: unsafe { core::rp_jmd_AcqGetRawBuffer(1) },
            chA_buff_float: Vec::with_capacity(16384),
            chB_buff_float: Vec::with_capacity(16384),
            chA_last_waveform: Vec::with_capacity(16384),
            chB_last_waveform: Vec::with_capacity(16384),
            region: ScopeRegion {
                skip_start: 0,
                skip_end: 0,
                skip_rate: 1,
                num_points: 16834,
            },
            _resource: &mut pit.scope_resource,
        }
    }

    /// Set the region-of-interest for this scope. When grabbing data from the scope,
    /// it will return a vector of the data in the acquisition buffer, but
    /// - Not the first ``skip_start`` points
    /// - Not the last ``skip_end`` points
    /// - Within that region, only every ``skip_rate``-th point
    pub fn set_roi(&mut self, skip_start: u32, skip_end: u32, skip_rate: u32) {
        let start_clamped = skip_start.clamp(0, 16383);
        let end_clamped = skip_end.clamp(0, 16383 - skip_start);
        let rate_clamped = skip_rate.clamp(1, 16383 - start_clamped - end_clamped);
        let num_points = ((16384 - start_clamped - end_clamped) + rate_clamped - 1) / rate_clamped;
        self.chA_buff_float = Vec::new();
        self.chA_buff_float.reserve_exact(num_points as usize);
        self.chB_buff_float = Vec::new();
        self.chB_buff_float.reserve_exact(num_points as usize);
        self.region = ScopeRegion {
            skip_start: start_clamped,
            skip_end: end_clamped,
            skip_rate: rate_clamped,
            num_points: num_points as usize,
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_decimation(&mut self, dec: u32) -> APIResult<()> {
        // decimation can be any of [1, 2, 4, 8, 16 -- 65536]
        let dec_factor;
        if dec == 1 {
            dec_factor = 1;
        } else if dec < 4 {
            dec_factor = 2;
        } else if dec < 8 {
            dec_factor = 4;
        } else if dec < 16 {
            dec_factor = 8;
        } else {
            dec_factor = dec;
        }

        if let Some(errcode) =
            APIError::from_i32(unsafe { core::rp_AcqSetDecimationFactor(dec_factor) })
        {
            match errcode {
                core::APIError::RP_OK => Ok(()),
                _ => Err(errcode),
            }
        } else {
            panic!();
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_trigger_source(&mut self, src: TrigSrc) -> APIResult<()> {
        match APIError::from_i32(unsafe {
            core::rp_AcqSetTriggerSrc(src as core::rp_acq_trig_src_t)
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
    pub fn get_trigger_state(&self) -> APIResult<TrigState> {
        let mut state = MaybeUninit::uninit();
        match APIError::from_i32(unsafe { core::rp_AcqGetTriggerState(state.as_mut_ptr()) })
            .unwrap()
        {
            RP_OK => Ok(TrigState::from_u32(unsafe { state.assume_init() }).unwrap()),
            error => Err(error),
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn set_trigger_delay(&mut self, delay: i32) -> APIResult<()> {
        // Sets the oscilloscope up to write (8192 + delay) points of data into the acquisition
        // buffer after the trigger. That is, calling with delay = 0 means the trigger is centered
        // in the data buffer, while (delay = 8192) means the whole buffer is written after the
        // trigger event.
        match APIError::from_i32(unsafe { core::rp_AcqSetTriggerDelay(delay) }).unwrap() {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn start_acquisition(&mut self) -> APIResult<()> {
        match APIError::from_i32(unsafe { core::rp_AcqStart() }).unwrap() {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn stop_acquisition(&mut self) -> APIResult<()> {
        match APIError::from_i32(unsafe { core::rp_AcqStop() }).unwrap() {
            RP_OK => Ok(()),
            error => Err(error),
        }
    }

    /// Returns a pair of vectors containing the most recent scope data (as u32) culled to
    /// `self`'s configured ROI. NOTE: allocates a pair of vectors
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn get_scope_data_both(&mut self) -> APIResult<(Vec<u32>, Vec<u32>)> {
        // returns owned vectors of the data in the region of interest described by self.region.
        // The API has functions for this, but only for copying the whole acq buffer, which is
        // slow, presumably because of memory bandwidth limitations. If we only use part of the
        // buffer, though, it makes more sense to only copy those parts of it.
        // I've previously implemented this by not copying buffers at all, and simply doing
        // direct access to the FPGA registers, but I believe it should be noticeably faster to
        // do a single read from the FPGA registers of the data we need, and then we can cache
        // those vectors while we do math on them.
        let index = self.get_write_index_at_trigger()?;
        let mut ret_a = Vec::with_capacity(self.region.num_points);
        let mut ret_b = Vec::with_capacity(self.region.num_points);
        for i in (self.region.skip_start..(16384 - self.region.skip_end))
            .step_by(self.region.skip_rate as usize)
        {
            ret_a.push(unsafe {
                read_volatile(
                    self.chA_buff_raw
                        .offset((index.wrapping_add(i)) as isize % 16384),
                )
            });
            ret_b.push(unsafe {
                read_volatile(
                    self.chB_buff_raw
                        .offset((index.wrapping_add(i)) as isize % 16384),
                )
            });
        }
        Ok((ret_a, ret_b))
    }

    /// updates the `Oscilloscope`'s internal buffers with most recent scope data.
    /// Provided as an alternative to `get_scope_data_both` that avoids heap allocation.
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// In case of an error, the state of the buffers is unspecified.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn update_scope_data_both(&mut self) -> APIResult<()> {
        let index = self.get_write_index_at_trigger()?;
        for i in (self.region.skip_start..(16384 - self.region.skip_end))
            .step_by(self.region.skip_rate as usize)
        {
            self.chA_buff_float[i as usize] = unsafe {
                read_volatile(
                    self.chA_buff_raw
                        .offset((index.wrapping_add(i)) as isize % 16384),
                ) as f32
            };
            self.chB_buff_float[i as usize] = unsafe {
                read_volatile(
                    self.chA_buff_raw
                        .offset((index.wrapping_add(i)) as isize % 16384),
                ) as f32
            };
        }
        Ok(())
    }

    /// Writes the most recent raw scope waveform into a pair of user-provided vectors. Vectors
    /// are user-provided so that the user can avoid unnecessary heap allocations.
    /// This version does not cull data down to the region of interest, and is intended to be
    /// used to send the full scope trace to an external monitoring program.
    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// In case of an error, the state of the buffers is unspecified.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn write_raw_waveform(&mut self, chA: &mut Vec<u32>, chB: &mut Vec<u32>) -> APIResult<()> {
        chA.reserve_exact(16384 - chA.len());
        chB.reserve_exact(16384 - chB.len());
        let index = self.get_write_index_at_trigger()?;
        for i in 0..16384 {
            chA[i as usize] = unsafe {
                read_volatile(
                    self.chA_buff_raw
                        .offset((index.wrapping_add(i)) as isize % 16384),
                )
            };
            chB[i as usize] = unsafe {
                read_volatile(
                    self.chA_buff_raw
                        .offset((index.wrapping_add(i)) as isize % 16384),
                )
            };
        }
        chA.truncate(16384);
        chB.truncate(16384);
        Ok(())
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    pub fn get_scope_data_channel(&mut self, ch: Channel) -> APIResult<Vec<u32>> {
        let index = self.get_write_index_at_trigger()?;
        let mut ret = Vec::with_capacity(self.region.num_points);
        for i in (self.region.skip_start..(16384 - self.region.skip_end))
            .step_by(self.region.skip_rate as usize)
        {
            ret.push(unsafe {
                read_volatile(
                    match ch {
                        Channel::CH_1 => self.chA_buff_raw,
                        Channel::CH_2 => self.chB_buff_raw,
                    }
                    .offset((index.wrapping_add(i)) as isize % 16384),
                )
            });
        }
        Ok(ret)
    }

    fn get_write_index_at_trigger(&mut self) -> APIResult<u32> {
        // While the pitaya acquires, it has an internal counter and it writes to the 16384-item-wide
        // buffer using the bottom 14 bits as an index, then increments the counter. That is, it
        // writes to the buffer in a cycle. This function returns the position of the most-recent
        // trigger event in the buffer, letting us "unwrap" the buffer into a waveform.
        // Note that this function returns the COUNTER, not the 14-bit position.
        let mut posn = MaybeUninit::uninit();
        match APIError::from_i32(unsafe { core::rp_AcqGetWritePointerAtTrig(posn.as_mut_ptr()) })
            .unwrap()
        {
            RP_OK => Ok(unsafe { posn.assume_init() }),
            error => Err(error),
        }
    }
}
