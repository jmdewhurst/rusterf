#![warn(clippy::pedantic)]
#![allow(non_snake_case)]

use std::fmt::{self, Debug};
use std::str::Split;

use itertools::{Itertools, MinMaxResult};

use crate::ring_buffer::DyadicRingBuffer;

#[derive(Debug, Default, Clone, Copy)]
pub enum EnableState {
    #[default]
    Unresolved,
    Locked,
}
#[derive(Debug, Default, Clone, Copy)]
pub enum DisableState {
    #[default]
    Unresolved,
    Resolved,
}
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Disabled(DisableState),
    Enabled(EnableState),
}
impl Default for Mode {
    fn default() -> Self {
        Mode::Disabled(DisableState::Unresolved)
    }
}
impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Mode::Enabled(_) => write!(f, "ENABLED"),
            Mode::Disabled(_) => write!(f, "DISABLED"),
        }
    }
}

#[derive(Debug)]
pub struct Servo {
    pub gain_P: f32,
    pub gain_I: f32,
    pub gain_D: f32,
    alpha_I: f32, // the 'decay rate' -- each step, the integral is multiplied by this value
    integral: f32,

    last_vals: DyadicRingBuffer<f32>,

    // approx. time between samples. In effect, the actual integral gain is
    // <gain_I * sample_time_sec> and actual derivative gain is
    // <gain_D / sample_time_sec>. There's no functional difference, but this
    // scheme is a bit more consistent with standard PID tuning methods like Ziegler-Nichols.
    pub sample_time_sec: Option<f32>,

    setpoint: f32,
    error_feedback: f32,
    mode: Mode,

    pub default_output_voltage: Option<f32>,
    pub max_feedback_step_size: f32,
    pub err_max_tolerance: f32,
}

impl Default for Servo {
    fn default() -> Self {
        Servo {
            gain_P: 0.0,
            gain_I: 0.0,
            gain_D: 0.0,
            alpha_I: 1.0,
            integral: 0.0,
            last_vals: DyadicRingBuffer::new(3)
                .expect("Should be able to allocate 8-element buffer"),
            sample_time_sec: None,
            setpoint: 0.0,
            error_feedback: 0.0,
            mode: Mode::Disabled(DisableState::Unresolved),
            max_feedback_step_size: f32::INFINITY,
            err_max_tolerance: f32::INFINITY,
            default_output_voltage: None,
        }
    }
}

impl Servo {
    pub fn do_pid(&mut self, new_error: f32) -> f32 {
        let err = if new_error.is_nan() {
            self.last_vals.last()
        } else {
            new_error
        };
        self.last_vals.push(err);
        self.update_state();
        self.pid_core(err)
            .clamp(-self.max_feedback_step_size, self.max_feedback_step_size)
    }

    fn pid_core(&mut self, err: f32) -> f32 {
        match self.mode {
            Mode::Enabled(EnableState::Locked) => {
                self.integral *= self.alpha_I;
                self.integral += err * self.sample_time_sec.unwrap_or(1.0);
                let deriv_term = self.gain_D * (err - self.last_vals.last())
                    / self.sample_time_sec.unwrap_or(1.0);
                let integral_term = self.gain_I * self.integral;
                err * self.gain_P + deriv_term + integral_term
            }
            Mode::Enabled(EnableState::Unresolved)
            | Mode::Disabled(DisableState::Unresolved | DisableState::Resolved) => 0.,
        }
    }

    fn update_state(&mut self) {
        match self.last_vals.iter().minmax() {
            MinMaxResult::OneElement(_) => {
                self.mode = match self.mode {
                    Mode::Enabled(_) => Mode::Enabled(EnableState::Locked),
                    Mode::Disabled(_) => Mode::Disabled(DisableState::Resolved),
                }
            }
            MinMaxResult::MinMax(x, y) if y - x < self.err_max_tolerance => {
                self.mode = match self.mode {
                    Mode::Enabled(_) => Mode::Enabled(EnableState::Locked),
                    Mode::Disabled(_) => Mode::Disabled(DisableState::Resolved),
                };
            }
            MinMaxResult::MinMax(_, _) => {
                self.mode = match self.mode {
                    Mode::Enabled(_) => Mode::Enabled(EnableState::Unresolved),
                    Mode::Disabled(_) => Mode::Disabled(DisableState::Unresolved),
                }
            }
            MinMaxResult::NoElements => {}
        }
    }

    pub fn enable(&mut self) {
        self.integral = 0.0;
        self.mode = match self.mode {
            Mode::Enabled(x) => Mode::Enabled(x),
            Mode::Disabled(DisableState::Resolved) => Mode::Enabled(EnableState::Locked),
            Mode::Disabled(DisableState::Unresolved) => Mode::Enabled(EnableState::Unresolved),
        };
    }
    pub fn disable(&mut self) {
        self.mode = match self.mode {
            Mode::Disabled(x) => Mode::Disabled(x),
            Mode::Enabled(EnableState::Unresolved) => Mode::Disabled(DisableState::Unresolved),
            Mode::Enabled(EnableState::Locked) => Mode::Disabled(DisableState::Resolved),
        }
    }
    #[inline]
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    #[inline]
    #[must_use]
    pub fn last_error(&self) -> f32 {
        self.last_vals.last()
    }

    pub fn set_alpha_I(&mut self, new_alpha: f32) {
        self.alpha_I = if new_alpha.is_nan() {
            1.0
        } else {
            new_alpha.min(1.0).max(0.0)
        };
    }

    #[must_use]
    #[inline]
    pub fn alpha_I(&self) -> f32 {
        self.alpha_I
    }

    #[inline]
    pub fn reset_integral(&mut self) {
        self.integral = 0.0;
    }

    #[inline]
    pub fn set_setpoint(&mut self, new_setpoint: f32) {
        if !new_setpoint.is_nan() {
            self.setpoint = new_setpoint;
        }
        self.integral = 0.0;
    }

    #[must_use]
    #[inline]
    pub fn setpoint(&self) -> f32 {
        self.setpoint
    }
    #[must_use]
    #[inline]
    pub fn error_feedback(&self) -> f32 {
        self.error_feedback
    }

    /// Takes a split over a string command, parses the command, executes the command, and returns
    /// a string
    /// # Errors
    /// In case of an invalid command (or inability to parse a command), returns an empty `Err`
    #[allow(clippy::result_unit_err)]
    pub fn process_command(&mut self, cmd: Split<'_, char>) -> Result<String, ()> {
        let resp = match cmd.collect::<Vec<&str>>()[..] {
            ["GAIN_P", "SET", x] => {
                self.gain_P = x.parse::<f32>().or(Err(()))?;
                String::new()
            }
            ["GAIN_P", "GET"] => self.gain_P.to_string(),
            ["GAIN_I", "SET", x] => {
                self.gain_I = x.parse::<f32>().or(Err(()))?;
                self.reset_integral();
                String::new()
            }
            ["GAIN_I", "GET"] => self.gain_I.to_string(),
            ["GAIN_D", "SET", x] => {
                self.gain_D = x.parse::<f32>().or(Err(()))?;
                String::new()
            }
            ["GAIN_D", "GET"] => self.gain_D.to_string(),
            ["ALPHA_I", "SET", x] => {
                self.set_alpha_I(x.parse::<f32>().or(Err(()))?);
                String::new()
            }
            ["ALPHA_I", "GET"] => self.alpha_I().to_string(),
            ["SETPOINT", "SET", x] => {
                self.set_setpoint(x.parse::<f32>().or(Err(()))?);
                String::new()
            }
            ["SETPOINT", "GET"] => self.setpoint().to_string(),
            ["MODE", "SET", "ENABLE"] => {
                self.enable();
                String::new()
            }
            ["MODE", "SET", "DISABLE"] => {
                self.disable();
                String::new()
            }
            ["MODE", "GET"] => self.mode.to_string(),
            ["MAX_STEP_SIZE", "SET", x] => {
                self.max_feedback_step_size = x.parse::<f32>().or(Err(()))?;
                String::new()
            }
            ["MAX_STEP_SIZE", "GET"] => self.max_feedback_step_size.to_string(),
            _ => Err(())?,
        };
        Ok(resp)
    }
}
