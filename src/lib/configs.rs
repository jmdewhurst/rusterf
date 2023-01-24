#![allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]
use std::f32::consts::PI;

use gethostname::gethostname;
use librp_sys::core::Channel;
use toml;

use crate::multifit::FitSetup;

use super::{communications::InterfComms, interferometer::Interferometer};

pub fn comms_from_config(cfg: &str, ctx: zmq::Context) -> Option<InterfComms> {
    let data: toml::Value = toml::from_str(cfg).ok()?;
    let mut out = InterfComms::new(ctx)?;
    out.bind_sockets(
        data["general"]["logs_port"].as_integer()? as u16,
        data["general"]["command_port"].as_integer()? as u16,
    )
    .ok()?;
    Some(out)
}

#[allow(clippy::too_many_lines)]
pub fn interferometer_from_config(cfg: &str) -> Option<Interferometer> {
    let data: toml::Value = toml::from_str(cfg).ok()?;
    let hostname = gethostname().into_string().ok()?;
    let hostname = hostname.as_str();

    let mut out = Interferometer::new()?;
    out.is_master = data[hostname]["is_master"].as_bool()?;

    // start by initializing the ramp setup:
    out.ramp_setup
        .amplitude(data["ramp"]["amplitude_volts"].as_float()? as f32);
    out.ramp_setup
        .preamp_gain(data["ramp"]["preamp_gain"].as_float()? as f32);
    out.ramp_setup
        .piezo_settle_time(data["ramp"]["piezo_settle_time_ms"].as_float()? as f32);
    out.ramp_setup
        .piezo_scale_factor(data["ramp"]["piezo_scale_factor"].as_float()? as f32);
    out.ramp_setup
        .decimation(data["ramp"]["decimation_factor"].as_integer()? as u32);

    // error buffers are of length 2^n, so we check if a valid exponent has been provided. If not, then we see if they've provided an explicit length, and round it down to a power of 2. If neither of those works, then we'll just use a default length of 1024 items.
    let buffer_size_exponent;
    if let Some(exponent) = data["general"]["pitaya_log_length_exponent"].as_integer() {
        buffer_size_exponent = exponent;
    } else if let Some(length) = data["general"]["pitaya_log_length"].as_integer() {
        buffer_size_exponent = (length as f32).log2().floor() as i64;
        eprintln!(
            "WARN: config explicit log length parameter rounded down to 2^{}.",
            buffer_size_exponent
        );
    } else {
        buffer_size_exponent = 10;
        eprintln!(
            "WARN: no log length parameter found in configuration file, using default of {}",
            2u32.pow(buffer_size_exponent as u32)
        );
    }

    // need to know which slave laser this machine is running:
    let slave_laser_name = data[hostname]["slave_laser"].as_str()?;

    // set the physical parameters of the lasers --- lock will come later
    out.ref_laser.set_wavelength(
        data["ref_laser"]["wavelength_nm"].as_float()? as f32,
        out.ramp_setup.piezo_scale_factor,
        out.ramp_setup.amplitude_volts,
    );
    match data[hostname]["ref_input_channel"].as_str()? {
        "CH_1" | "CH_A" => out.ref_laser.input_channel = Channel::CH_1,
        "CH_2" | "CH_B" => out.ref_laser.input_channel = Channel::CH_2,
        _ => {
            eprintln!("No valid input channel for reference laser found");
            return None;
        }
    };
    match data[hostname]["ref_output_channel"].as_str()? {
        "CH_1" | "CH_A" => out.ref_laser.output_channel = Channel::CH_1,
        "CH_2" | "CH_B" => out.ref_laser.output_channel = Channel::CH_2,
        _ => {
            eprintln!("No valid output channel for reference laser found");
            return None;
        }
    };
    out.slave_laser.set_wavelength(
        data[slave_laser_name]["wavelength_nm"].as_float()? as f32,
        out.ramp_setup.piezo_scale_factor,
        out.ramp_setup.amplitude_volts,
    );
    match data[hostname]["slave_input_channel"].as_str()? {
        "CH_1" | "CH_A" => out.slave_laser.input_channel = Channel::CH_1,
        "CH_2" | "CH_B" => out.slave_laser.input_channel = Channel::CH_2,
        _ => {
            eprintln!(
                "No valid input channel for laser {} found",
                slave_laser_name
            );
            return None;
        }
    };
    match data[hostname]["slave_output_channel"].as_str()? {
        "CH_1" | "CH_A" => out.slave_laser.output_channel = Channel::CH_1,
        "CH_2" | "CH_B" => out.slave_laser.output_channel = Channel::CH_2,
        _ => {
            eprintln!(
                "No valid output channel for laser {} found",
                slave_laser_name
            );
            return None;
        }
    };

    // set up the phase error logs for the lasers
    out.ref_laser
        .resize_logs(buffer_size_exponent as usize)
        .ok()?;
    out.slave_laser
        .resize_logs(buffer_size_exponent as usize)
        .ok()?;

    // fill in ``guess'' fit coefficients for the lasers
    let freq_base = out.ramp_setup.piezo_scale_factor * out.ramp_setup.amplitude_volts / 16384.0;
    out.ref_laser.fit_coefficients = [
        1000.,
        freq_base / out.ref_laser.wavelength_nm() / 2.0 / PI,
        0.,
        3000.,
    ];
    out.slave_laser.fit_coefficients = [
        1000.,
        freq_base / out.slave_laser.wavelength_nm() / 2.0 / PI,
        0.,
        3000.,
    ];

    // now set up servo lock parameters
    out.ref_lock.gain_P = data["ref_laser"]["gain_p"].as_float()? as f32;
    out.ref_lock.gain_I = data["ref_laser"]["gain_p"].as_float()? as f32;
    out.ref_lock.gain_D = data["ref_laser"]["gain_p"].as_float()? as f32;
    out.ref_lock
        .set_alpha_I(data["ref_laser"]["integral_decay_rate"].as_float()? as f32);

    out.slave_lock.gain_P = data[slave_laser_name]["gain_p"].as_float()? as f32;
    out.slave_lock.gain_I = data[slave_laser_name]["gain_p"].as_float()? as f32;
    out.slave_lock.gain_D = data[slave_laser_name]["gain_p"].as_float()? as f32;
    out.slave_lock
        .set_alpha_I(data[slave_laser_name]["integral_decay_rate"].as_float()? as f32);

    // now we configure the fitting algorithm
    let fit = &data["multifit"];
    let num_points = (16384 - fit["skip_start"].as_integer()? - fit["skip_end"].as_integer()?
        + fit["skip_rate"].as_integer()?
        - 1)
        / fit["skip_rate"].as_integer()?;
    out.fit_setup = FitSetup::init(
        fit["skip_rate"].as_integer()? as u32,
        num_points as u32,
        fit["max_iterations"].as_integer()? as u32,
        fit["xtol"].as_float()? as f32,
        fit["gtol"].as_float()? as f32,
        fit["ftol"].as_float()? as f32,
        fit["max_av_ratio"].as_float()? as f32,
    )?;

    Some(out)
}
