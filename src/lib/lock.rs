#![warn(clippy::pedantic)]
#![allow(non_snake_case)]

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Default)]
pub enum Mode {
    Enabled,
    #[default]
    Disabled,
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
    pub max_feedback_value: f32,
    pub min_feedback_value: f32,
}

impl Servo {
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
        let adjustment = self
            .pid_core(err)
            .clamp(-self.max_feedback_step_size, self.max_feedback_step_size);
        self.error_feedback = (self.error_feedback + adjustment)
            .clamp(self.min_feedback_value, self.max_feedback_step_size);
        self.error_feedback
    }

    fn pid_core(&mut self, err: f32) -> f32 {
        match self.mode {
            Mode::Enabled => {
                self.integral *= self.alpha_I;
                let deriv_term;
                if let Some(x) = self.sample_time_sec {
                    self.integral += err * x;
                    deriv_term = self.gain_D * (err - self.last_error) / x;
                } else {
                    self.integral += err;
                    deriv_term = self.gain_D * (err - self.last_error);
                }
                let integral_term = self.gain_I * self.integral;
                err * self.gain_P + deriv_term + integral_term
            }
            Mode::Disabled => 0.,
        }
    }

    pub fn enable(&mut self) {
        self.mode = Mode::Enabled;
    }

    pub fn disable(&mut self) {
        self.mode = Mode::Disabled;
    }

    pub fn set_alpha_I(&mut self, new_alpha: f32) {
        self.alpha_I = if new_alpha.is_nan() {
            1.0
        } else {
            new_alpha.min(1.0).max(0.0)
        };
    }

    pub fn alpha_I(&self) -> f32 {
        self.alpha_I
    }

    pub fn reset_integral(&mut self) {
        self.integral = 0.0;
    }

    pub fn set_setpoint(&mut self, new_setpoint: f32) {
        if !new_setpoint.is_nan() {
            self.setpoint = new_setpoint;
        }
        self.integral = 0.0;
    }

    pub fn setpoint(&self) -> f32 {
        self.setpoint
    }
    pub fn error_feedback(&self) -> f32 {
        self.error_feedback
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
    max_feedback_value: f32,
    min_feedback_value: f32,
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
        out.max_feedback_value = self.max_feedback_value;
        out.min_feedback_value = self.min_feedback_value;
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
            max_feedback_value: lock.max_feedback_value,
            min_feedback_value: lock.min_feedback_value,
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
