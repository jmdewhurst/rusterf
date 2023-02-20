#![warn(clippy::pedantic)]
#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::Split;

#[derive(Debug, Default)]
pub enum Mode {
    #[default]
    Disabled,
    Enabled,
}
impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Enabled => write!(f, "Enabled"),
            Mode::Disabled => write!(f, "Disabled"),
        }
    }
}
#[derive(Debug, Default)]
pub struct Servo {
    pub gain_P: f32,
    pub gain_I: f32,
    pub gain_D: f32,
    alpha_I: f32, // the 'decay rate' -- each step, the integral is multiplied by this value
    integral: f32,
    last_error: f32,

    // approx. time between samples. In effect, the actual integral gain is
    // <gain_I * sample_time_sec> and actual derivative gain is
    // <gain_D / sample_time_sec>. There's no functional difference, but this
    // scheme is a bit more consistent with standard PID tuning methods like Ziegler-Nichols.
    pub sample_time_sec: Option<f32>,

    setpoint: f32,
    error_feedback: f32,
    pub mode: Mode,

    pub max_feedback_step_size: f32,
}

impl Servo {
    #[must_use]
    pub fn new() -> Self {
        Servo {
            ..Default::default()
        }
    }

    pub fn do_pid(&mut self, new_error: f32) -> f32 {
        let err = if new_error.is_nan() {
            self.last_error
        } else {
            new_error
        };
        let out = self
            .pid_core(err)
            .clamp(-self.max_feedback_step_size, self.max_feedback_step_size);
        self.last_error = err;
        out
    }

    fn pid_core(&mut self, err: f32) -> f32 {
        match self.mode {
            Mode::Enabled => {
                self.integral *= self.alpha_I;
                self.integral += err * self.sample_time_sec.unwrap_or(1.0);
                let deriv_term =
                    self.gain_D * (err - self.last_error) / self.sample_time_sec.unwrap_or(1.0);
                let integral_term = self.gain_I * self.integral;
                err * self.gain_P + deriv_term + integral_term
            }
            Mode::Disabled => 0.,
        }
    }

    #[inline]
    pub fn enable(&mut self) {
        self.mode = Mode::Enabled;
    }

    #[inline]
    pub fn disable(&mut self) {
        self.mode = Mode::Disabled;
    }

    #[inline]
    #[must_use]
    pub fn last_error(&self) -> f32 {
        self.last_error
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
    pub fn process_command(&mut self, cmd: Split<'_, char>) -> Result<Option<String>, ()> {
        match cmd.collect::<Vec<&str>>()[..] {
            ["GAIN_P", "SET", x] => {
                self.gain_P = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["GAIN_P", "GET"] => Ok(Some(self.gain_P.to_string())),
            ["GAIN_I", "SET", x] => {
                self.gain_I = x.parse::<f32>().map_err(|_| ())?;
                self.reset_integral();
                Ok(None)
            }
            ["GAIN_I", "GET"] => Ok(Some(self.gain_I.to_string())),
            ["GAIN_D", "SET", x] => {
                self.gain_D = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["GAIN_D", "GET"] => Ok(Some(self.gain_D.to_string())),
            ["ALPHA_I", "SET", x] => {
                self.set_alpha_I(x.parse::<f32>().map_err(|_| ())?);
                Ok(None)
            }
            ["ALPHA_I", "GET"] => Ok(Some(self.alpha_I().to_string())),
            ["SETPOINT", "SET", x] => {
                self.set_setpoint(x.parse::<f32>().map_err(|_| ())?);
                Ok(None)
            }
            ["SETPOINT", "GET"] => Ok(Some(self.setpoint().to_string())),
            ["MODE", "SET", "ENABLE"] => {
                self.enable();
                Ok(None)
            }
            ["MODE", "SET", "DISABLE"] => {
                self.disable();
                Ok(None)
            }
            ["MODE", "GET"] => Ok(Some(self.mode.to_string())),
            ["MAX_STEP_SIZE", "SET", x] => {
                self.max_feedback_step_size = x.parse::<f32>().map_err(|_| ())?;
                Ok(None)
            }
            ["MAX_STEP_SIZE", "GET"] => Ok(Some(self.max_feedback_step_size.to_string())),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct LockSerialize {
    gain_P: f32,
    gain_I: f32,
    gain_D: f32,
    alpha_I: f32,

    // approx. time between samples. In effect, the actual integral gain is
    // <gain_I * sample_time_sec> and actual derivative gain is
    // <gain_D / sample_time_sec>. There's no functional difference, but this
    // scheme is a bit more consistent with standard PID tuning methods like Ziegler-Nichols.
    sample_time_sec: Option<f32>,

    max_feedback_step_size: f32,
}

impl LockSerialize {
    fn into_servo(self) -> Servo {
        let mut out = Servo::new();
        out.gain_P = self.gain_P;
        out.gain_I = self.gain_I;
        out.gain_D = self.gain_D;
        out.set_alpha_I(self.alpha_I);
        out.sample_time_sec = self.sample_time_sec;
        out.max_feedback_step_size = self.max_feedback_step_size;
        out
    }

    fn from_servo(lock: &Servo) -> Self {
        LockSerialize {
            gain_P: lock.gain_P,
            gain_I: lock.gain_I,
            gain_D: lock.gain_D,
            alpha_I: lock.alpha_I(),
            sample_time_sec: lock.sample_time_sec,
            max_feedback_step_size: lock.max_feedback_step_size,
        }
    }
}

impl<'de> Deserialize<'de> for Servo {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(LockSerialize::deserialize(d)?.into_servo())
    }
}

impl Serialize for Servo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        LockSerialize::from_servo(self).serialize(serializer)
    }
}
