#[warn(clippy::pedantic)]
#[allow(non_snake_case)]
#[derive(Debug, Default)]
pub enum Mode {
    Enabled,
    #[default]
    Disabled,
}
#[derive(Debug, Default)]
pub struct Servo {
    pub gain_P: f32,
    pub gain_I: f32,
    pub gain_D: f32,
    pub alpha_I: f32, // the 'decay rate' -- each step, the integral is multiplied by this value
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

    pub fn do_pid(&mut self, new_error: f32) {
        let err = if new_error.is_nan() {
            self.last_error
        } else {
            new_error
        };
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
                self.error_feedback += err * self.gain_P + deriv_term + integral_term;
            }
            Mode::Disabled => {}
        }
        self.last_error = err;
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
