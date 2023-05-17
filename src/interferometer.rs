#![warn(clippy::pedantic)]
#![allow(clippy::result_unit_err)]

use std::num::NonZeroU32;
use std::str::Split;
use std::thread;
use std::time::{Duration, Instant, SystemTimeError, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use librp_sys::core::{APIError, APIResult, RPCoreChannel};
use librp_sys::generator::{Channel, Pulse, DC};
use librp_sys::oscilloscope::Oscilloscope;

use super::laser::{ReferenceLaser, SlaveLaser};
use super::lock::Servo;
use super::ramp::DaqSetup;
use crate::multifit;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct State {
    pub ref_position_setpoint: f32,
    pub slave_position_setpoint: f32,
    pub slave_setpoint: f32,
    pub slave_output_v: f32,
    pub slave_locked: bool,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub enum ApplyStateError {
    RPError(APIError),
    ClockError(SystemTimeError),
    OutdatedState,
    InvalidState,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Statistics {
    pub avg_fitting_time_us: u32,

    pub avg_err_ref: f32,
    pub variance_ref: f32,
    pub avg_iterations_ref: f32,

    pub avg_err_slave: f32,
    pub variance_slave: f32,
    pub avg_iterations_slave: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct CumulativeStatistics {
    averaging: NonZeroU32,
    stats: Statistics,
}

impl Default for CumulativeStatistics {
    fn default() -> Self {
        Self {
            averaging: NonZeroU32::new(1).unwrap(),
            stats: Default::default(),
        }
    }
}

impl CumulativeStatistics {
    #[must_use]
    pub fn new(averaging: u32) -> Self {
        Self {
            averaging: NonZeroU32::new(averaging).unwrap_or_else(|| NonZeroU32::new(1).unwrap()),
            ..Self::default()
        }
    }

    pub fn set_averaging(&mut self, averaging: u32) {
        self.averaging = NonZeroU32::new(averaging).unwrap_or_else(|| NonZeroU32::new(1).unwrap());
        self.stats = Default::default();
    }

    pub fn reset(&mut self) {
        self.stats = Default::default();
    }

    pub fn new_time_us(&mut self, new_time: u32) -> &mut Self {
        self.stats.avg_fitting_time_us += new_time;
        self
    }
    pub fn new_errs(&mut self, new_err_ref: f32, new_err_slave: f32) -> &mut Self {
        self.stats.avg_err_ref += new_err_ref;
        self.stats.variance_ref += new_err_ref * new_err_ref;
        self.stats.avg_err_slave += new_err_slave;
        self.stats.variance_slave += new_err_slave * new_err_slave;
        self
    }
    pub fn new_iterations(&mut self, iter_ref: u32, iter_slave: u32) -> &mut Self {
        self.stats.avg_iterations_ref += iter_ref as f32;
        self.stats.avg_iterations_slave += iter_slave as f32;
        self
    }

    pub fn evaluate(&mut self) -> Statistics {
        Statistics {
            avg_fitting_time_us: self.stats.avg_fitting_time_us / self.averaging,
            avg_err_ref: self.stats.avg_err_ref / self.averaging.get() as f32,
            variance_ref: self.stats.variance_ref / self.averaging.get() as f32,
            avg_iterations_ref: self.stats.avg_iterations_ref / self.averaging.get() as f32,
            avg_err_slave: self.stats.avg_err_slave / self.averaging.get() as f32,
            variance_slave: self.stats.variance_slave / self.averaging.get() as f32,
            avg_iterations_slave: self.stats.avg_iterations_slave / self.averaging.get() as f32,
        }
    }
}

#[derive(Debug)]
pub struct Interferometer {
    pub ref_laser: ReferenceLaser,
    pub ref_position_lock: Servo,
    pub slave_laser: SlaveLaser,
    pub slave_position_lock: Servo,
    pub slave_servo: Servo,
    pub fit_setup_ref: multifit::FitSetup,
    pub fit_setup_slave: multifit::FitSetup,
    pub stats: CumulativeStatistics,

    pub ramp_setup: DaqSetup,
    pub cycle_counter: u64,
    pub last_waveform_ref: Vec<u32>,
    pub last_waveform_slave: Vec<u32>,

    pub do_swap_file: bool,
    pub start_time: Instant,
}

impl Interferometer {
    #[must_use]
    pub fn new() -> Option<Self> {
        Some(Interferometer {
            ref_laser: ReferenceLaser::new(8)?,
            ref_position_lock: Servo::default(),
            slave_laser: SlaveLaser::new(8)?,
            slave_position_lock: Servo::default(),
            slave_servo: Servo::default(),
            fit_setup_ref: multifit::FitSetup::new(10).init()?,
            fit_setup_slave: multifit::FitSetup::new(10).init()?,
            stats: Default::default(),

            ramp_setup: DaqSetup::new(),
            cycle_counter: 0,
            last_waveform_ref: Vec::with_capacity(16384),
            last_waveform_slave: Vec::with_capacity(16384),
            do_swap_file: false,
            start_time: Instant::now(),
        })
    }
    #[inline]
    #[must_use]
    pub fn is_master(&self) -> bool {
        self.ref_laser.output_channel.is_some()
    }

    fn update_fringe_params(&mut self) {
        self.ref_laser.set_wavelength(
            self.ref_laser.wavelength_nm(),
            self.ramp_setup.piezo_scale_factor,
            self.ramp_setup.amplitude_volts,
        );
        self.slave_laser.set_wavelength(
            self.slave_laser.wavelength_nm(),
            self.ramp_setup.piezo_scale_factor,
            self.ramp_setup.amplitude_volts,
        );

        self.ref_position_lock.reset_integral();
        self.slave_servo.reset_integral();
    }

    /// Copy the data from the Red Pitaya's internal oscilloscope buffer into the buffers of `self`.
    /// # Errors
    /// Propagates any Red Pitaya API errors
    pub fn update_last_waveforms(&mut self, osc: &mut Oscilloscope) -> APIResult<()> {
        match self.ref_laser.input_channel {
            RPCoreChannel::CH_1 => {
                osc.write_raw_waveform(&mut self.last_waveform_ref, &mut self.last_waveform_slave)?;
            }
            RPCoreChannel::CH_2 => {
                osc.write_raw_waveform(&mut self.last_waveform_slave, &mut self.last_waveform_ref)?;
            }
        };
        Ok(())
    }

    fn process_ramp_command(
        &mut self,
        cmd: Split<'_, char>,
        ramp_ch: Option<&mut Channel<'_, Pulse>>,
    ) -> Result<String, ()> {
        let resp = match cmd.collect::<Vec<&str>>()[..] {
            ["AMPL", "SET", x] => {
                self.ramp_setup.amplitude(x.parse::<f32>().or(Err(()))?);
                self.update_fringe_params();
                format!(
                    "{:?}",
                    ramp_ch.map(|x| x.set_amplitude(self.ramp_setup.amplitude_volts))
                )
            }
            ["AMPL", "GET"] => self.ramp_setup.amplitude_volts.to_string(),
            ["SCALE_FACTOR", "SET", x] => {
                self.ramp_setup.piezo_scale_factor = x.parse::<f32>().or(Err(()))?;
                self.update_fringe_params();
                String::new()
            }
            ["SCALE_FACTOR", "GET"] => self.ramp_setup.piezo_scale_factor.to_string(),
            ["SETTLE_TIME", "SET", x] => {
                self.ramp_setup
                    .piezo_settle_time_ms(x.parse::<f32>().or(Err(()))?);
                String::new()
            }
            ["SETTLE_TIME", "GET"] => self.ramp_setup.piezo_settle_time_ms.to_string(),
            _ => Err(())?,
        };
        Ok(resp)
    }

    fn process_laser_command(&mut self, cmd: Split<'_, char>) -> Result<String, ()> {
        let resp = match cmd.collect::<Vec<&str>>()[..] {
            ["REF", "WAVELENGTH", "SET", x] => {
                self.ref_laser.set_wavelength(
                    x.parse::<f32>().or(Err(()))?,
                    self.ramp_setup.piezo_scale_factor,
                    self.ramp_setup.amplitude_volts,
                );
                String::new()
            }
            ["REF", "WAVELENGTH", "GET"] => self.ref_laser.wavelength_nm().to_string(),
            ["SLAVE", "WAVELENGTH", "SET", x] => {
                self.slave_laser.set_wavelength(
                    x.parse::<f32>().or(Err(()))?,
                    self.ramp_setup.piezo_scale_factor,
                    self.ramp_setup.amplitude_volts,
                );
                String::new()
            }
            ["SLAVE", "WAVELENGTH", "GET"] => self.slave_laser.wavelength_nm().to_string(),
            _ => Err(())?,
        };
        Ok(resp)
    }

    /// Handle an incoming command by routing it to the appropriate sufunction. Returns a String
    /// holding the response to the sender of the command.
    /// # Errors
    /// Returns `Err(())` in case of a failure to parse a valid command in `cmd`
    pub fn process_command(
        &mut self,
        mut cmd: Split<'_, char>,
        ramp_ch: Option<&mut Channel<'_, Pulse>>,
        slave_ch: &mut Channel<'_, DC>,
    ) -> Result<String, ()> {
        match cmd.next() {
            Some("RAMP") => self.process_ramp_command(cmd, ramp_ch),
            Some("LASER") => self.process_laser_command(cmd),
            Some("LOCK") => match cmd.next() {
                Some("REF") => self.ref_position_lock.process_command(cmd),
                Some("SLAVE") => self.slave_servo.process_command(cmd),
                Some(_) | None => Err(()),
            },
            Some("OUTPUT") => match cmd.next() {
                Some("SET") => {
                    slave_ch
                        .set_offset(cmd.next().and_then(|x| x.parse::<f32>().ok()).ok_or(())?)
                        .map_err(|_| ())?;
                    self.slave_servo.reset_integral();
                    Ok(String::new())
                }
                Some(_) | None => Err(()),
            },
            Some(_) | None => Err(()),
        }
    }

    pub fn state(&self) -> Result<State, SystemTimeError> {
        Ok(State {
            ref_position_setpoint: self.ref_position_lock.setpoint(),
            slave_position_setpoint: self.slave_position_lock.setpoint(),
            slave_setpoint: self.slave_servo.setpoint(),
            slave_output_v: self.slave_laser.feedback_log.last(),
            slave_locked: match self.slave_servo.mode() {
                crate::lock::Mode::Enabled(_) => true,
                crate::lock::Mode::Disabled(_) => false,
            },
            timestamp: UNIX_EPOCH.elapsed()?.as_secs(),
        })
    }

    pub fn apply_state(
        &mut self,
        state: State,
        slave_ch: &mut Channel<'_, DC>,
    ) -> Result<(), ApplyStateError> {
        // refuse to accept states that are more than four days old
        if UNIX_EPOCH
            .elapsed()
            .map_err(|x| ApplyStateError::ClockError(x))?
            .as_secs()
            > state.timestamp + 3600 * 24 * 4
        {
            return Err(ApplyStateError::OutdatedState);
        }
        let (min, max) = slave_ch.output_range_v();
        if state.slave_output_v < min || state.slave_output_v > max {
            return Err(ApplyStateError::InvalidState);
        }
        while slave_ch.offset() != state.slave_output_v {
            slave_ch
                .adjust_offset((state.slave_output_v - slave_ch.offset()).clamp(-0.1, 0.1))
                .map_err(|x| ApplyStateError::RPError(x))?;
            thread::sleep(Duration::from_millis(250));
        }
        self.ref_position_lock
            .set_setpoint(state.ref_position_setpoint);
        self.slave_position_lock.set_setpoint(state.slave_position_setpoint);
        self.slave_servo.set_setpoint(state.slave_setpoint);
        if state.slave_locked {
            self.slave_servo.enable();
        }
        Ok(())
    }
}
