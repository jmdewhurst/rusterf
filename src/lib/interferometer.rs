#![warn(clippy::pedantic)]

use std::str::Split;

use librp_sys::core::{APIResult, Channel};
use librp_sys::oscilloscope::Oscilloscope;

use super::laser::Laser;
use super::lock::Servo;
use super::ramp::DaqSetup;
use crate::multifit;

#[derive(Debug)]
pub struct Interferometer {
    pub ref_laser: Laser,
    pub ref_lock: Servo,
    pub slave_laser: Laser,
    pub slave_lock: Servo,
    pub fit_setup_ref: multifit::FitSetup,
    pub fit_setup_slave: multifit::FitSetup,

    pub ramp_setup: DaqSetup,
    pub cycle_counter: u64,
    pub last_waveform_ref: Vec<u32>,
    pub last_waveform_slave: Vec<u32>,
}

impl Interferometer {
    #[must_use]
    pub fn new() -> Option<Self> {
        Some(Interferometer {
            ref_laser: Laser::new(12)?,
            ref_lock: Servo::new(),
            slave_laser: Laser::new(12)?,
            slave_lock: Servo::new(),
            fit_setup_ref: multifit::FitSetup::init(1, 16384, 16, 1e-6, 1e-6, 1e-6, 3.0)?,
            fit_setup_slave: multifit::FitSetup::init(1, 16384, 16, 1e-6, 1e-6, 1e-6, 3.0)?,

            ramp_setup: DaqSetup::new(),
            cycle_counter: 0,
            last_waveform_ref: Vec::with_capacity(16384),
            last_waveform_slave: Vec::with_capacity(16384),
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

        self.ref_lock.reset_integral();
        self.slave_lock.reset_integral();
    }

    pub fn update_last_waveforms(&mut self, osc: &mut Oscilloscope) -> APIResult<()> {
        match self.ref_laser.input_channel {
            Channel::CH_1 => {
                osc.write_raw_waveform(&mut self.last_waveform_ref, &mut self.last_waveform_slave)?;
            }
            Channel::CH_2 => {
                osc.write_raw_waveform(&mut self.last_waveform_slave, &mut self.last_waveform_ref)?;
            }
        };
        Ok(())
    }

    fn process_ramp_command(&mut self, cmd: Split<'_, char>) -> Result<Option<String>, ()> {
        match cmd.collect::<Vec<&str>>()[..] {
            ["AMPL", "SET", x] => {
                self.ramp_setup.amplitude(x.parse::<f32>().map_err(|_| ())?);
                self.update_fringe_params();
                Ok(None)
            }
            ["AMPL", "GET"] => Ok(Some(self.ramp_setup.amplitude_volts.to_string())),
            ["SCALE_FACTOR", "SET", x] => {
                self.ramp_setup.piezo_scale_factor = x.parse::<f32>().map_err(|_| ())?;
                self.update_fringe_params();
                Ok(None)
            }
            ["SCALE_FACTOR", "GET"] => Ok(Some(self.ramp_setup.piezo_scale_factor.to_string())),
            ["SETTLE_TIME", "SET", x] => {
                self.ramp_setup
                    .piezo_settle_time_ms(x.parse::<f32>().map_err(|_| ())?);
                Ok(None)
            }
            ["SETTLE_TIME", "GET"] => Ok(Some(self.ramp_setup.piezo_settle_time_ms.to_string())),
            _ => Err(()),
        }
    }

    fn process_laser_command(&mut self, cmd: Split<'_, char>) -> Result<Option<String>, ()> {
        match cmd.collect::<Vec<&str>>()[..] {
            ["REF", "WAVELENGTH", "SET", x] => {
                self.ref_laser.set_wavelength(
                    x.parse::<f32>().map_err(|_| ())?,
                    self.ramp_setup.piezo_scale_factor,
                    self.ramp_setup.amplitude_volts,
                );
                Ok(None)
            }
            ["REF", "WAVELENGTH", "GET"] => Ok(Some(self.ref_laser.wavelength_nm().to_string())),
            ["SLAVE", "WAVELENGTH", "SET", x] => {
                self.slave_laser.set_wavelength(
                    x.parse::<f32>().map_err(|_| ())?,
                    self.ramp_setup.piezo_scale_factor,
                    self.ramp_setup.amplitude_volts,
                );
                Ok(None)
            }
            ["SLAVE", "WAVELENGTH", "GET"] => {
                Ok(Some(self.slave_laser.wavelength_nm().to_string()))
            }
            _ => Err(()),
        }
    }

    pub fn process_command(&mut self, mut cmd: Split<'_, char>) -> Result<Option<String>, ()> {
        match cmd.next() {
            Some("RAMP") => self.process_ramp_command(cmd),
            Some("LASER") => self.process_laser_command(cmd),
            Some("LOCK") => match cmd.next() {
                Some("REF") => {
                    if self.is_master() {
                        self.ref_lock.process_command(cmd)
                    } else {
                        Err(())
                    }
                }
                Some("SLAVE") => self.slave_lock.process_command(cmd),
                Some(_) | None => Err(()),
            },
            Some(_) | None => Err(()),
        }
    }
}
