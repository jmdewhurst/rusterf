#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::unused_self)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]

use crate::core;
use crate::core::{APIError, APIError::RP_OK, APIResult, Channel};
use enum_primitive::*;
use std::ptr::read_volatile;

// Red pitaya samples at 125 MHz
pub const BASE_SAMPLE_RATE: f32 = 125_000_000.0;

// The oscilloscope buffer is 16384 points
pub const BUFF_SIZE: usize = 16384;
// bitmask to get the lower 14 bits; bitwise AND with this mask
// is equivalent to division by 16384 but is much more performant.
// This relies on the fact that Red Pitaya's internal oscilloscope buffers are aligned to 16484
// bytes; this will not work for a general ring buffer.
pub const BUFF_MASK: usize = 16384 - 1;

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
    skip_start: usize,
    skip_end: usize,
    skip_rate: usize,
    num_points: usize,
}

#[derive(Debug)]
pub struct Oscilloscope {
    chA_buff_raw: *const u32,
    chB_buff_raw: *const u32,
    // maintains arrays of recent scope data, culled to ``region`` and converted to floating pt
    pub chA_buff_float: Vec<f32>,
    pub chB_buff_float: Vec<f32>,
    // arrays of a FULL waveform, as the raw u32, for caching a waveform to send over a socket
    pub chA_last_waveform: Vec<u32>,
    pub chB_last_waveform: Vec<u32>,
    region: ScopeRegion,
}

/// # Errors
/// If an RP API call returns a failure code, this returns Err containing the failure.
/// # Panics
/// Panics if the RP API returns a catastrophically wrong value
impl Oscilloscope {
    #[must_use]
    pub(crate) fn init() -> Self {
        Oscilloscope {
            chA_buff_raw: unsafe { core::rp_jmd_AcqGetRawBuffer(0) },
            chB_buff_raw: unsafe { core::rp_jmd_AcqGetRawBuffer(1) },
            chA_buff_float: Vec::with_capacity(BUFF_SIZE),
            chB_buff_float: Vec::with_capacity(BUFF_SIZE),
            chA_last_waveform: Vec::with_capacity(BUFF_SIZE),
            chB_last_waveform: Vec::with_capacity(BUFF_SIZE),
            region: ScopeRegion {
                skip_start: 0,
                skip_end: 0,
                skip_rate: 1,
                num_points: 16834,
            },
        }
    }

    /// Set the region-of-interest for this scope. When grabbing data from the scope,
    /// it will return a vector of the data in the acquisition buffer, but
    /// - Not the first ``skip_start`` points
    /// - Not the last ``skip_end`` points
    /// - Within that region, only every ``skip_rate``-th point
    pub fn set_roi(&mut self, skip_start: usize, skip_end: usize, skip_rate: usize) {
        let start_clamped = skip_start.clamp(0, 16383);
        let end_clamped = skip_end.clamp(0, 16383 - skip_start);
        let rate_clamped = skip_rate.clamp(1, 16383 - start_clamped - end_clamped);
        let num_points =
            ((BUFF_SIZE - start_clamped - end_clamped) + rate_clamped - 1) / rate_clamped;
        self.chA_buff_float = Vec::new();
        self.chA_buff_float.reserve_exact(num_points);
        self.chB_buff_float = Vec::new();
        self.chB_buff_float.reserve_exact(num_points);
        self.region = ScopeRegion {
            skip_start: start_clamped,
            skip_end: end_clamped,
            skip_rate: rate_clamped,
            num_points,
        }
    }

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

        if dec != dec_factor {
            eprintln!("Attempting to set invalid decimation factor {dec}! Valid decimation factors are 1, 2, 4, 8, or any value between 16 and 65536. Proceeding with decimation factor of {dec_factor}");
        }

        wrap_call!(rp_AcqSetDecimationFactor, dec_factor)
    }

    #[inline]
    pub fn set_trigger_source(&mut self, src: TrigSrc) -> APIResult<()> {
        wrap_call!(rp_AcqSetTriggerSrc, src as core::rp_acq_trig_src_t)
    }

    #[inline]
    pub fn get_trigger_state(&self) -> APIResult<TrigState> {
        let mut trig_state = 0;
        wrap_call!(rp_AcqGetTriggerState, std::ptr::addr_of_mut!(trig_state),)?;
        Ok(unsafe { TrigState::from_u32(trig_state).unwrap_unchecked() })
    }

    /// Sets the oscilloscope up to write (8192 + delay) points of data into the acquisition
    /// buffer after the trigger. That is, calling with delay = 0 means the trigger is centered
    /// in the data buffer, while (delay = 8192) means the whole buffer is written after the
    /// trigger event.
    #[inline]
    pub fn set_trigger_delay(&mut self, delay: i32) -> APIResult<()> {
        wrap_call!(rp_AcqSetTriggerDelay, delay)
    }

    #[inline]
    pub fn start_acquisition(&mut self) -> APIResult<()> {
        wrap_call!(rp_AcqStart)
    }

    #[inline]
    pub fn stop_acquisition(&mut self) -> APIResult<()> {
        wrap_call!(rp_AcqStop)
    }

    /// Returns a pair of vectors containing the most recent scope data (as u32) culled to
    /// `self`'s configured ROI. NOTE: allocates a pair of vectors
    #[allow(clippy::unnecessary_cast)]
    pub fn get_scope_data_both(&mut self) -> APIResult<(Vec<u32>, Vec<u32>)> {
        // returns owned vectors of the data in the region of interest described by self.region.
        // The API has functions for this, but only for copying the whole acq buffer, which is
        // slow, presumably because of memory bandwidth limitations. If we only use part of the
        // buffer, though, it makes more sense to only copy those parts of it.
        // I've previously implemented this by not copying buffers at all, and simply doing
        // direct access to the FPGA registers, but I believe it should be noticeably faster to
        // do a single read from the FPGA registers of the data we need, and then we can cache
        // those vectors while we do math on them.
        let index = self.get_write_index_at_trigger()? as isize;
        let mut ret_a = Vec::with_capacity(self.region.num_points);
        let mut ret_b = Vec::with_capacity(self.region.num_points);
        for i in (self.region.skip_start..(BUFF_SIZE - self.region.skip_end))
            .step_by(self.region.skip_rate)
        {
            ret_a.push(unsafe {
                read_volatile(
                    self.chA_buff_raw
                        .offset((index.wrapping_add(i as isize + 1)) as isize & BUFF_MASK as isize),
                )
            });
            ret_b.push(unsafe {
                read_volatile(
                    self.chB_buff_raw
                        .offset((index.wrapping_add(i as isize + 1)) as isize & BUFF_MASK as isize),
                )
            });
        }
        Ok((ret_a, ret_b))
    }

    /// updates the `Oscilloscope`'s internal buffers with most recent scope data.
    /// Provided as an alternative to `get_scope_data_both` that avoids heap allocation.
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::unnecessary_cast)]
    pub fn update_scope_data_both(&mut self) -> APIResult<()> {
        let index = self.get_write_index_at_trigger()? as isize;

        self.chA_buff_float.clear();
        self.chB_buff_float.clear();
        self.chA_buff_float.reserve_exact(self.region.num_points);
        self.chB_buff_float.reserve_exact(self.region.num_points);

        let region_iter = (self.region.skip_start..(BUFF_SIZE - self.region.skip_end))
            .step_by(self.region.skip_rate);
        self.chA_buff_float.extend(region_iter.map(|i| unsafe {
            read_volatile(
                self.chA_buff_raw
                    .offset((index.wrapping_add(i as isize + 1)) as isize & BUFF_MASK as isize),
            ) as f32
        }));

        let region_iter = (self.region.skip_start..(BUFF_SIZE - self.region.skip_end))
            .step_by(self.region.skip_rate as usize);
        self.chB_buff_float.extend(region_iter.map(|i| unsafe {
            read_volatile(
                self.chB_buff_raw
                    .offset((index.wrapping_add(i as isize + 1)) as isize & BUFF_MASK as isize),
            ) as f32
        }));

        Ok(())
    }

    /// Writes the most recent raw scope waveform into a pair of user-provided vectors. Vectors
    /// are user-provided so that the user can avoid unnecessary heap allocations.
    /// This version does not cull data down to the region of interest, and is intended to be
    /// used to send the full scope trace to an external monitoring program.
    #[allow(clippy::unnecessary_cast)]
    pub fn write_raw_waveform(&mut self, chA: &mut Vec<u32>, chB: &mut Vec<u32>) -> APIResult<()> {
        let index = self.get_write_index_at_trigger()? as isize;
        chA.clear();
        chB.clear();
        chA.reserve_exact(BUFF_SIZE);
        chB.reserve_exact(BUFF_SIZE);

        chA.extend((0..BUFF_SIZE).map(|i| unsafe {
            read_volatile(
                self.chA_buff_raw
                    .offset((index.wrapping_add(i as isize + 1)) as isize & BUFF_MASK as isize),
            )
        }));
        chB.extend((0..BUFF_SIZE).map(|i| unsafe {
            read_volatile(
                self.chB_buff_raw
                    .offset((index.wrapping_add(i as isize + 1)) as isize & BUFF_MASK as isize),
            )
        }));

        Ok(())
    }

    /// # Errors
    /// If an RP API call returns a failure code, this returns Err containing the failure.
    /// # Panics
    /// Panics if the RP API returns a catastrophically wrong value
    #[allow(clippy::unnecessary_cast)]
    pub fn get_scope_data_channel(&mut self, ch: Channel) -> APIResult<Vec<u32>> {
        let index = self.get_write_index_at_trigger()? as isize;
        let mut ret = Vec::with_capacity(self.region.num_points);
        for i in (self.region.skip_start..(BUFF_SIZE - self.region.skip_end))
            .step_by(self.region.skip_rate as usize)
        {
            ret.push(unsafe {
                read_volatile(
                    match ch {
                        Channel::CH_1 => self.chA_buff_raw,
                        Channel::CH_2 => self.chB_buff_raw,
                    }
                    .offset((index.wrapping_add(i as isize + 1)) as isize & BUFF_MASK as isize),
                )
            });
        }
        Ok(ret)
    }

    /// While the pitaya acquires, it has an internal counter and it writes to the 16384-item-wide
    /// buffer using the bottom 14 bits as an index, then increments the counter. That is, it
    /// writes to the buffer in a cycle. This function returns the position of the most-recent
    /// trigger event in the buffer, letting us "unwrap" the buffer into a waveform.
    /// Note that this function returns the 32-bit COUNTER, not the 14-bit position.
    fn get_write_index_at_trigger(&mut self) -> APIResult<u32> {
        let mut posn: u32 = 0;
        wrap_call!(rp_AcqGetWritePointerAtTrig, std::ptr::addr_of_mut!(posn),)?;
        Ok(posn)
    }
}
