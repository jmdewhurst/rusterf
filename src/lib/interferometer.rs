#![warn(clippy::pedantic)]

use std::str::Split;

use super::laser::Laser;
use super::lock::Servo;
use super::ramp::Ramp;
use crate::multifit;

#[derive(Debug)]
pub struct Interferometer {
    pub is_master: bool,
    pub ref_laser: Laser,
    pub ref_lock: Servo,
    pub slave_laser: Laser,
    pub slave_lock: Servo,
    pub fit_setup: multifit::FitSetup,

    pub ramp_setup: Ramp,
    pub cycle_counter: u64,
}

impl Interferometer {
    pub fn new() -> Option<Self> {
        Some(Interferometer {
            is_master: true,
            ref_laser: Laser::new(14)?,
            ref_lock: Servo::new(),
            slave_laser: Laser::new(14)?,
            slave_lock: Servo::new(),
            fit_setup: multifit::FitSetup::init(1, 16384, 16, 1e-6, 1e-6, 1e-6, 3.0)?,

            ramp_setup: Ramp::new(),
            cycle_counter: 0,
        })
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

    #[allow(clippy::too_many_lines)]
    pub fn process_command(&mut self, cmd: Split<'_, char>) -> Result<Option<String>, ()> {
        let cmds_split: Vec<&str> = cmd.collect();
        match &cmds_split[..] {
            // -----------------------------------------------------------------------------
            //   Voltage ramp options
            // -----------------------------------------------------------------------------
            ["RAMP", "AMPL", "SET", x] => {
                self.ramp_setup
                    .set_amplitude(x.parse::<f32>().map_err(|_| ())?);
                self.update_fringe_params();
                Ok(None)
            }
            ["RAMP", "AMPL", "GET"] => Ok(Some(self.ramp_setup.amplitude_volts.to_string())),
            ["RAMP", "GAIN", "SET", x] => {
                self.ramp_setup
                    .set_preamp_gain(x.parse::<f32>().map_err(|_| ())?);
                self.update_fringe_params();
                Ok(None)
            }
            ["RAMP", "GAIN", "GET"] => Ok(Some(self.ramp_setup.preamp_gain.to_string())),
            ["RAMP", "SCALE_FACTOR", "SET", x] => {
                self.ramp_setup.piezo_scale_factor = x.parse::<f32>().map_err(|_| ())?;
                self.update_fringe_params();
                Ok(None)
            }
            ["RAMP", "SCALE_FACTOR", "GET"] => {
                Ok(Some(self.ramp_setup.piezo_scale_factor.to_string()))
            }
            ["RAMP", "SETTLE_TIME", "SET", x] => {
                self.ramp_setup
                    .set_piezo_settle_time(x.parse::<f32>().map_err(|_| ())?);
                Ok(None)
            }
            ["RAMP", "SETTLE_TIME", "GET"] => {
                Ok(Some(self.ramp_setup.piezo_settle_time_ms.to_string()))
            }
            // -----------------------------------------------------------------------------
            //   Reference laser general options
            // -----------------------------------------------------------------------------
            ["REF", "LASER", "WAVELENGTH", "SET", x] => {
                self.ref_laser.set_wavelength(
                    x.parse::<f32>().map_err(|_| ())?,
                    self.ramp_setup.piezo_scale_factor,
                    self.ramp_setup.amplitude_volts,
                );
                Ok(None)
            }
            ["REF", "LASER", "WAVELENGTH", "GET"] => {
                Ok(Some(self.ref_laser.wavelength_nm().to_string()))
            }
            // -----------------------------------------------------------------------------
            //   Slave laser general options
            // -----------------------------------------------------------------------------
            ["SLAVE", "LASER", "WAVELENGTH", "SET", x] => {
                self.slave_laser.set_wavelength(
                    x.parse::<f32>().map_err(|_| ())?,
                    self.ramp_setup.piezo_scale_factor,
                    self.ramp_setup.amplitude_volts,
                );
                Ok(None)
            }
            ["SLAVE", "LASER", "WAVELENGTH", "GET"] => {
                Ok(Some(self.slave_laser.wavelength_nm().to_string()))
            }
            // -----------------------------------------------------------------------------
            //   Reference laser lock options
            // -----------------------------------------------------------------------------
            ["REF", "LOCK", "GAIN_P", "SET", x] => {
                self.ref_lock.gain_P = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["REF", "LOCK", "GAIN_P", "GET"] => Ok(Some(self.ref_lock.gain_P.to_string())),
            ["REF", "LOCK", "GAIN_I", "SET", x] => {
                self.ref_lock.gain_I = x.parse::<f32>().map_err(|_| ())?;
                self.ref_lock.reset_integral();
                Ok(None)
            }
            ["REF", "LOCK", "GAIN_I", "GET"] => Ok(Some(self.ref_lock.gain_I.to_string())),
            ["REF", "LOCK", "GAIN_D", "SET", x] => {
                self.ref_lock.gain_D = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["REF", "LOCK", "GAIN_D", "GET"] => Ok(Some(self.ref_lock.gain_D.to_string())),
            ["REF", "LOCK", "ALPHA_I", "SET", x] => {
                self.ref_lock.set_alpha_I(x.parse::<f32>().map_err(|_| ())?);
                Ok(None)
            }
            ["REF", "LOCK", "ALPHA_I", "GET"] => Ok(Some(self.ref_lock.alpha_I().to_string())),
            ["REF", "LOCK", "SETPOINT", "SET", x] => {
                self.ref_lock
                    .set_setpoint(x.parse::<f32>().map_err(|_| ())?);
                Ok(None)
            }
            ["REF", "LOCK", "SETPOINT", "GET"] => Ok(Some(self.ref_lock.setpoint().to_string())),
            ["REF", "LOCK", "MODE", "SET", "ENABLE"] => {
                self.ref_lock.enable();
                Ok(None)
            }
            ["REF", "LOCK", "MODE", "SET", "DISABLE"] => {
                self.ref_lock.disable();
                Ok(None)
            }
            ["REF", "LOCK", "MODE", "GET"] => Ok(Some(self.ref_lock.mode.to_string())),
            ["REF", "LOCK", "MAX_FEEDBACK_VALUE", "SET", x] => {
                self.ref_lock.max_feedback_value = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["REF", "LOCK", "MAX_FEEDBACK_VALUE", "GET"] => {
                Ok(Some(self.ref_lock.max_feedback_value.to_string()))
            }
            ["REF", "LOCK", "MIN_FEEDBACK_VALUE", "SET", x] => {
                self.ref_lock.min_feedback_value = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["REF", "LOCK", "MIN_FEEDBACK_VALUE", "GET"] => {
                Ok(Some(self.ref_lock.min_feedback_value.to_string()))
            }
            ["REF", "LOCK", "MAX_STEP_SIZE", "SET", x] => {
                self.ref_lock.max_feedback_step_size = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["REF", "LOCK", "MAX_STEP_SIZE", "GET"] => {
                Ok(Some(self.ref_lock.max_feedback_step_size.to_string()))
            }
            // -----------------------------------------------------------------------------
            //   Slave laser lock options
            // -----------------------------------------------------------------------------
            ["SLAVE", "LOCK", "GAIN_P", "SET", x] => {
                self.slave_lock.gain_P = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["SLAVE", "LOCK", "GAIN_P", "GET"] => Ok(Some(self.slave_lock.gain_P.to_string())),
            ["SLAVE", "LOCK", "GAIN_I", "SET", x] => {
                self.slave_lock.gain_I = x.parse::<f32>().map_err(|_| ())?;
                self.slave_lock.reset_integral();
                Ok(None)
            }
            ["SLAVE", "LOCK", "GAIN_I", "GET"] => Ok(Some(self.slave_lock.gain_I.to_string())),
            ["SLAVE", "LOCK", "GAIN_D", "SET", x] => {
                self.slave_lock.gain_D = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["SLAVE", "LOCK", "GAIN_D", "GET"] => Ok(Some(self.slave_lock.gain_D.to_string())),
            ["SLAVE", "LOCK", "ALPHA_I", "SET", x] => {
                self.slave_lock
                    .set_alpha_I(x.parse::<f32>().map_err(|_| ())?);
                Ok(None)
            }
            ["SLAVE", "LOCK", "ALPHA_I", "GET"] => Ok(Some(self.slave_lock.alpha_I().to_string())),
            ["SLAVE", "LOCK", "SETPOINT", "SET", x] => {
                self.slave_lock
                    .set_setpoint(x.parse::<f32>().map_err(|_| ())?);
                Ok(None)
            }
            ["SLAVE", "LOCK", "SETPOINT", "GET"] => {
                Ok(Some(self.slave_lock.setpoint().to_string()))
            }
            ["SLAVE", "LOCK", "MODE", "SET", "ENABLE"] => {
                self.slave_lock.enable();
                Ok(None)
            }
            ["SLAVE", "LOCK", "MODE", "SET", "DISABLE"] => {
                self.slave_lock.disable();
                Ok(None)
            }
            ["SLAVE", "LOCK", "MODE", "GET"] => Ok(Some(self.slave_lock.mode.to_string())),
            ["SLAVE", "LOCK", "MAX_FEEDBACK_VALUE", "SET", x] => {
                self.slave_lock.max_feedback_value = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["SLAVE", "LOCK", "MAX_FEEDBACK_VALUE", "GET"] => {
                Ok(Some(self.slave_lock.max_feedback_value.to_string()))
            }
            ["SLAVE", "LOCK", "MIN_FEEDBACK_VALUE", "SET", x] => {
                self.slave_lock.min_feedback_value = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["SLAVE", "LOCK", "MIN_FEEDBACK_VALUE", "GET"] => {
                Ok(Some(self.slave_lock.min_feedback_value.to_string()))
            }
            ["SLAVE", "LOCK", "MAX_STEP_SIZE", "SET", x] => {
                self.slave_lock.max_feedback_step_size = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["SLAVE", "LOCK", "MAX_STEP_SIZE", "GET"] => {
                Ok(Some(self.slave_lock.max_feedback_step_size.to_string()))
            }
            _ => Err(()),
        }
    }
}
