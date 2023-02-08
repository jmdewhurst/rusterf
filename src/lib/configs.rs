#![allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]

use gethostname::gethostname;
use std::str::FromStr;
use toml;

use librp_sys::dpin::DigitalPin;
use librp_sys::generator::Generator;
use librp_sys::oscilloscope::Oscilloscope;
use librp_sys::{core, dpin};

use crate::multifit::FitSetup;

use super::laser::Laser;
use super::lock::Servo;
use super::ramp::DaqSetup;
use super::{communications::InterfComms, interferometer::Interferometer};
//TODO: Replace arguments of (&str) with (&mut toml::Value)?

// #[allow(clippy::unnecessary_wraps)]
pub fn generator_from_config(cfg: &toml::Value, gen: &mut Generator) -> Option<()> {
    let hostname = gethostname().into_string().ok()?;
    let hostname = hostname.as_str();
    gen.ch_a.set_hw_offset_v(
        cfg.get(hostname)?
            .get("ch_1_out_hardware_offset_volts")?
            .as_float()? as f32,
    );
    gen.ch_a
        .set_gain_post(cfg.get(hostname)?.get("ch_1_preamp_gain")?.as_float()? as f32);
    gen.ch_a.set_output_range(
        cfg.get(hostname)?.get("ch_1_min_output_v")?.as_float()? as f32,
        cfg.get(hostname)?.get("ch_1_max_output_v")?.as_float()? as f32,
    );
    gen.ch_a
        .set_offset_v((gen.ch_a.max_output_v() - gen.ch_a.min_output_v()) / 2.0);
    gen.ch_b.set_hw_offset_v(
        cfg.get(hostname)?
            .get("ch_2_out_hardware_offset_volts")?
            .as_float()? as f32,
    );
    gen.ch_b
        .set_gain_post(cfg.get(hostname)?.get("ch_2_preamp_gain")?.as_float()? as f32);
    gen.ch_b.set_output_range(
        cfg.get(hostname)?.get("ch_2_min_output_v")?.as_float()? as f32,
        cfg.get(hostname)?.get("ch_2_max_output_v")?.as_float()? as f32,
    );
    gen.ch_b
        .set_offset_v((gen.ch_b.max_output_v() - gen.ch_b.min_output_v()) / 2.0);
    Some(())
}

#[must_use]
pub fn dpin_get_ready_pin(cfg: &toml::Value) -> Option<dpin::Pin> {
    cfg.get("general")?
        .get("ready_to_acquire_pin")?
        .as_str()
        .map(dpin::Pin::from_str)?
        .ok()
}
#[must_use]
pub fn dpin_get_trigger_pin(cfg: &toml::Value) -> Option<dpin::Pin> {
    cfg.get("general")?
        .get("master_external_trigger_output_pin")?
        .as_str()
        .map(dpin::Pin::from_str)?
        .ok()
}

pub fn dpin_from_config(cfg: &toml::Value, dpin: &mut DigitalPin) -> Option<()> {
    let hostname = gethostname().into_string().ok()?;
    let hostname = hostname.as_str();
    let is_master = cfg.get(hostname)?.get("is_master")?.as_bool()?;
    if is_master {
        let trigger_out_pin = cfg
            .get("general")?
            .get("master_external_trigger_output_pin")?
            .as_str()
            .map(dpin::Pin::from_str)?
            .ok()?;
        dpin.set_direction(trigger_out_pin, dpin::PinDirection::Out)
            .ok()?;
    };
    let ready_to_acquire_pin = cfg
        .get("general")?
        .get("ready_to_acquire_pin")?
        .as_str()
        .map(dpin::Pin::from_str)?
        .ok()?;
    dpin.set_direction(
        ready_to_acquire_pin,
        if is_master {
            dpin::PinDirection::In
        } else {
            dpin::PinDirection::Out
        },
    )
    .ok()?;
    // set external trigger pin as an input
    // TODO: Check if this is actually necessary? i.e., can it trigger on external even if
    // that pin is set as an output?
    dpin.set_direction(
        librp_sys::dpin::Pin::DIO0_P,
        librp_sys::dpin::PinDirection::In,
    )
    .ok()?;
    Some(())
}

pub fn scope_from_config(cfg: &toml::Value, scope: &mut Oscilloscope) -> Option<()> {
    let mfit = &cfg.get("multifit")?;
    scope.set_roi(
        mfit.get("samples_skip_start")?.as_integer()? as usize,
        mfit.get("samples_skip_end")?.as_integer()? as usize,
        mfit.get("skip_rate")?.as_integer()? as usize,
    );
    // NOTE: ramp::apply() also sets the decimation, waveform; we may be needlessly duplicating logic here
    scope
        .set_decimation(cfg.get("ramp")?.get("decimation_factor")?.as_integer()? as u32)
        .ok()?;
    scope
        .set_trigger_source(librp_sys::oscilloscope::TrigSrc::ExtRising)
        .ok()?;
    scope.set_trigger_delay(8192).ok()?;
    scope.start_acquisition().ok()?;
    Some(())
}

#[must_use]
pub fn comms_from_config(cfg: &str, ctx: &zmq::Context) -> Option<InterfComms> {
    let data: toml::Value = toml::from_str(cfg).ok()?;
    let mut out = InterfComms::new(ctx)?;
    let logs_port = data.get("general")?.get("logs_port")?.as_integer()? as u16;
    let command_port = data.get("general")?.get("command_port")?.as_integer()? as u16;
    if let Err(x) = out.bind_sockets(logs_port, command_port) {
        eprintln!(
            "Error [{}] in binding sockets to ports {}, {}",
            x, logs_port, command_port
        );
        return None;
    }
    Some(out)
}

#[must_use]
pub fn ramp_from_config(cfg: &toml::Value) -> Option<DaqSetup> {
    let mut out = DaqSetup::new();
    out.amplitude(cfg.get("ramp")?.get("amplitude_volts")?.as_float()? as f32);
    out.piezo_settle_time_ms(cfg.get("ramp")?.get("piezo_settle_time_ms")?.as_float()? as f32);
    out.piezo_scale_factor(cfg.get("ramp")?.get("piezo_scale_factor")?.as_float()? as f32);
    let dec_factor;
    let dec = cfg.get("ramp")?.get("decimation_factor")?.as_integer()? as u32;
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
        eprintln!("Decimation factor specified in config file as {}. Valid decimation factors are 1, 2, 4, 8, or any value between 16 and 65536. Proceeding with decimation factor of {}", dec, dec_factor);
    }
    out.set_decimation(dec_factor);
    Some(out)
}

#[must_use]
fn buff_size_exponent(cfg: &toml::Value) -> usize {
    // error buffers are of length 2^n, so we check if a valid exponent has been provided. If not, then we see if they've provided an explicit length, and round it down to a power of 2. If neither of those works, then we'll just use a default length of 1024 items.
    if let Some(exponent) = cfg
        .get("general")
        .and_then(|x| x.get("pitaya_log_length_exponent"))
        .and_then(toml::Value::as_integer)
    {
        exponent as usize
    } else if let Some(length) = cfg
        .get("general")
        .and_then(|x| x.get("pitaya_log_length"))
        .and_then(toml::Value::as_integer)
    {
        let buffer_size_exponent = (length as f32).log2().floor() as usize;
        eprintln!(
            "WARN: config explicit log length parameter {} rounded down to 2^{} = {}.",
            length,
            buffer_size_exponent,
            1 << buffer_size_exponent,
        );
        buffer_size_exponent
    } else {
        let buffer_size_exponent = 10;
        eprintln!(
            "WARN: no log length parameter found in configuration file, using default of {}",
            2u32.pow(buffer_size_exponent as u32)
        );
        buffer_size_exponent
    }
}

#[must_use]
pub fn ref_laser_from_config(cfg: &toml::Value) -> Option<Laser> {
    let hostname = gethostname().into_string().ok()?;
    let hostname = hostname.as_str();
    let is_master = cfg.get(hostname)?.get("is_master")?.as_bool()?;

    let buffer_size_exponent = buff_size_exponent(cfg);

    let mut out = Laser::new(buffer_size_exponent as usize)?;
    out.set_wavelength(
        cfg.get("ref_laser")?.get("wavelength_nm")?.as_float()? as f32,
        cfg.get("ramp")?.get("piezo_scale_factor")?.as_float()? as f32,
        cfg.get("ramp")?.get("amplitude_volts")?.as_float()? as f32,
    );
    match cfg.get(hostname)?.get("ref_input_channel")?.as_str()? {
        "CH_1" | "CH_A" => out.input_channel = core::Channel::CH_1,
        "CH_2" | "CH_B" => out.input_channel = core::Channel::CH_2,
        _ => {
            eprintln!("No valid input channel for reference laser found");
            return None;
        }
    };
    if is_master {
        match cfg.get(hostname)?.get("ref_output_channel")?.as_str()? {
            "CH_1" | "CH_A" => out.output_channel = Some(core::Channel::CH_1),
            "CH_2" | "CH_B" => out.output_channel = Some(core::Channel::CH_2),
            _ => {
                eprintln!("No valid output channel for reference laser found");
                return None;
            }
        };
    } else {
        out.output_channel = None;
    }

    // fill in ``guess'' fit coefficients for the lasers
    out.fit_coefficients = [0.0, out.fringe_freq(), 0.0, 1000.0];
    Some(out)
}

#[must_use]
pub fn slave_laser_from_config(cfg: &toml::Value) -> Option<Laser> {
    let hostname = gethostname().into_string().ok()?;
    let hostname = hostname.as_str();
    let buffer_size_exponent = buff_size_exponent(cfg);
    let mut out = Laser::new(buffer_size_exponent as usize)?;
    let slave_laser_name = cfg.get(hostname)?.get("slave_laser")?.as_str()?;
    out.set_wavelength(
        cfg.get(slave_laser_name)?
            .get("wavelength_nm")?
            .as_float()? as f32,
        cfg.get("ramp")?.get("piezo_scale_factor")?.as_float()? as f32,
        cfg.get("ramp")?.get("amplitude_volts")?.as_float()? as f32,
    );
    match cfg.get(hostname)?.get("slave_input_channel")?.as_str()? {
        "CH_1" | "CH_A" => out.input_channel = core::Channel::CH_1,
        "CH_2" | "CH_B" => out.input_channel = core::Channel::CH_2,
        _ => {
            eprintln!("No valid input channel for slave laser found");
            return None;
        }
    };
    match cfg.get(hostname)?.get("slave_output_channel")?.as_str()? {
        "CH_1" | "CH_A" => out.output_channel = Some(core::Channel::CH_1),
        "CH_2" | "CH_B" => out.output_channel = Some(core::Channel::CH_2),
        _ => {
            eprintln!("No valid output channel for slave laser found");
            return None;
        }
    };

    // fill in ``guess'' fit coefficients for the lasers
    out.fit_coefficients = [0.0, out.fringe_freq(), 0.0, 1000.0];
    Some(out)
}

#[must_use]
pub fn ref_lock_from_config(cfg: &toml::Value) -> Option<Servo> {
    let hostname = gethostname().into_string().ok()?;
    let hostname = hostname.as_str();
    let is_master = cfg.get(hostname)?.get("is_master")?.as_bool()?;
    let mut out = Servo::new();
    if is_master {
        out.gain_P = cfg.get("ref_laser")?.get("gain_p")?.as_float()? as f32;
        out.gain_I = cfg.get("ref_laser")?.get("gain_i")?.as_float()? as f32;
        out.gain_D = cfg.get("ref_laser")?.get("gain_d")?.as_float()? as f32;
        out.set_alpha_I(
            cfg.get("ref_laser")?
                .get("integral_decay_rate")?
                .as_float()? as f32,
        );
        out.max_feedback_step_size = cfg
            .get("ref_laser")?
            .get("feedback_max_step_size_v")?
            .as_float()? as f32;
        println!("alpha");
    }
    Some(out)
}
#[must_use]
pub fn slave_lock_from_config(cfg: &toml::Value) -> Option<Servo> {
    let hostname = gethostname().into_string().ok()?;
    let hostname = hostname.as_str();
    let slave_laser_name = cfg.get(hostname)?.get("slave_laser")?.as_str()?;
    let mut out = Servo::new();

    out.gain_P = cfg.get(slave_laser_name)?.get("gain_p")?.as_float()? as f32;
    out.gain_I = cfg.get(slave_laser_name)?.get("gain_i")?.as_float()? as f32;
    out.gain_D = cfg.get(slave_laser_name)?.get("gain_d")?.as_float()? as f32;
    out.set_alpha_I(
        cfg.get(slave_laser_name)?
            .get("integral_decay_rate")?
            .as_float()? as f32,
    );
    out.max_feedback_step_size = cfg
        .get(slave_laser_name)?
        .get("feedback_max_step_size_v")?
        .as_float()? as f32;
    Some(out)
}

#[must_use]
pub fn multifit_from_config(cfg: &toml::Value) -> Option<FitSetup> {
    let fit = &cfg.get("multifit")?;
    let num_points = (16384
        - fit.get("samples_skip_start")?.as_integer()?
        - fit.get("samples_skip_end")?.as_integer()?
        + fit.get("skip_rate")?.as_integer()?
        - 1)
        / fit.get("skip_rate")?.as_integer()?;

    FitSetup::init(
        fit.get("skip_rate")?.as_integer()? as u32,
        num_points as u32,
        fit.get("max_iterations")?.as_integer()? as u32,
        fit.get("xtol")?.as_float()? as f32,
        fit.get("gtol")?.as_float()? as f32,
        fit.get("ftol")?.as_float()? as f32,
        fit.get("max_av_ratio")?.as_float()? as f32,
    )
}

#[must_use]
pub fn interferometer_from_config(cfg: &str) -> Option<Interferometer> {
    let data: toml::Value = toml::from_str(cfg).ok()?;

    let mut out = Interferometer::new()?;

    out.ramp_setup = ramp_from_config(&data)?;
    out.ref_laser = ref_laser_from_config(&data)?;
    out.slave_laser = slave_laser_from_config(&data)?;
    println!("test");
    out.ref_lock = ref_lock_from_config(&data)?;
    println!("test");
    out.slave_lock = slave_lock_from_config(&data)?;
    out.fit_setup_ref = multifit_from_config(&data)?;
    out.fit_setup_slave = multifit_from_config(&data)?;
    Some(out)
}
