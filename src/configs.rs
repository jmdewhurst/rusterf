#![allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::missing_errors_doc
)]
#![allow(non_snake_case)]

use gethostname::gethostname;
use std::f32::consts::PI;
use std::str::FromStr;
use toml;

use librp_sys::dpin::DigitalPin;
use librp_sys::generator::Generator;
use librp_sys::oscilloscope::Oscilloscope;
use librp_sys::{core, dpin};

use crate::multifit::FitSetup;
use crate::util::tomlget_opt;

use super::laser::{ReferenceLaser, SlaveLaser};
use super::lock::Servo;
use super::ramp::DaqSetup;
use super::util::{tomlget, tomlget_or};
use super::{communications::InterfComms, interferometer::Interferometer};

pub fn generator_from_config(cfg: &toml::Value, gen: &mut Generator) -> Result<(), String> {
    let hostname = gethostname()
        .into_string()
        .map_err(|_| "failed to get hostname")?;
    let hostname = hostname.as_str();
    gen.ch_a.set_hw_offset_v(tomlget_or!(
        cfg,
        hostname,
        "ch_1_out_hardware_offset_volts",
        as_float,
        f32,
        0.0
    ));
    gen.ch_a.set_gain_post(tomlget_or!(
        cfg,
        hostname,
        "ch_1_preamp_gain",
        as_float,
        f32,
        1.0
    ));
    gen.ch_a.set_output_range(
        tomlget_or!(cfg, hostname, "ch_1_min_output_v", as_float, f32, -1.0),
        tomlget_or!(cfg, hostname, "ch_1_max_output_v", as_float, f32, 1.0),
    );
    let _ = gen
        .ch_a
        .set_trigger_source(librp_sys::generator::GenTriggerSource::ExternalRisingEdge);
    // gen.ch_a.enable();
    gen.ch_b.set_hw_offset_v(tomlget_or!(
        cfg,
        hostname,
        "ch_2_out_hardware_offset_volts",
        as_float,
        f32,
        0.0
    ));
    gen.ch_b.set_gain_post(tomlget_or!(
        cfg,
        hostname,
        "ch_2_preamp_gain",
        as_float,
        f32,
        1.0
    ));
    gen.ch_b.set_output_range(
        tomlget_or!(cfg, hostname, "ch_2_min_output_v", as_float, f32, -1.0),
        tomlget_or!(cfg, hostname, "ch_2_max_output_v", as_float, f32, 1.0),
    );
    let _ = gen
        .ch_b
        .set_trigger_source(librp_sys::generator::GenTriggerSource::ExternalRisingEdge);
    // gen.ch_b.enable();
    Ok(())
}

pub fn dpin_get_ready_pin(cfg: &toml::Value) -> Result<dpin::Pin, String> {
    dpin::Pin::from_str(tomlget_or!(
        cfg,
        "general",
        "ready_to_acquire_pin",
        as_str,
        "DIO7_P"
    ))
    .map_err(|_| "failed to convert to pin".into())
}
pub fn dpin_get_trigger_pin(cfg: &toml::Value) -> Result<dpin::Pin, String> {
    dpin::Pin::from_str(tomlget_or!(
        cfg,
        "general",
        "master_external_trigger_output_pin",
        as_str,
        "DIO6_P"
    ))
    .map_err(|_| "failed to convert to pin".into())
}

pub fn dpin_from_config(cfg: &toml::Value, dpin: &mut DigitalPin) -> Result<(), String> {
    let hostname = gethostname()
        .into_string()
        .map_err(|_| "failed to get hostname")?;
    let hostname = hostname.as_str();
    let is_master = tomlget_or!(cfg, hostname, "is_master", as_bool, false);
    dpin.set_all_input().expect("RP API call failure");
    if is_master {
        dpin.set_direction(dpin_get_trigger_pin(cfg)?, dpin::PinDirection::Out)
            .expect("RP API call failure");
    };
    dpin.set_direction(
        dpin_get_ready_pin(cfg)?,
        if is_master {
            dpin::PinDirection::In
        } else {
            dpin::PinDirection::Out
        },
    )
    .expect("RP API call failure");
    // set external trigger pin as an input
    // TODO: Check if this is actually necessary? i.e., can it trigger on external even if
    // that pin is set as an output?
    dpin.set_direction(
        librp_sys::dpin::Pin::DIO0_P,
        librp_sys::dpin::PinDirection::In,
    )
    .expect("RP API call failure");
    Ok(())
}

pub fn scope_from_config(cfg: &toml::Value, scope: &mut Oscilloscope) -> Result<(), String> {
    scope.set_roi(
        tomlget_or!(
            cfg,
            "multifit",
            "samples_skip_start",
            as_integer,
            usize,
            6000
        ),
        tomlget_or!(cfg, "multifit", "samples_skip_end", as_integer, usize, 0),
        tomlget_or!(cfg, "multifit", "skip_rate", as_integer, usize, 40),
    );
    // NOTE: ramp::apply() also sets the decimation, waveform; we may be needlessly duplicating logic here
    scope
        .set_decimation(tomlget_or!(
            cfg,
            "ramp",
            "decimation_factor",
            as_integer,
            u32,
            16
        ))
        .expect("RP API call failure");
    scope.set_trigger_delay(8192).expect("RP API call failure");
    scope
        .set_trigger_source(librp_sys::oscilloscope::TrigSrc::ExtRising)
        .expect("RP API call failure");
    scope.start_acquisition().expect("RP API call failure");
    Ok(())
}

pub async fn comms_from_config(cfg: &toml::Value) -> Result<InterfComms, String> {
    let mut out = InterfComms::new().ok_or("failed to instantiate comms struct")?;
    out.bind_sockets(
        tomlget_or!(cfg, "general", "logs_port", as_integer, u16, 8080),
        tomlget_or!(cfg, "general", "command_port", as_integer, u16, 8081),
    )
    .await
    .map_err(|e| format!("error [{}] in binding sockets", e))?;
    out.set_log_publish_frequency(tomlget_or!(
        cfg,
        "general",
        "logs_publish_freq_cycles",
        as_integer,
        u32,
        256
    ));
    Ok(out)
}

pub fn ramp_from_config(cfg: &toml::Value) -> Result<DaqSetup, String> {
    let mut out = DaqSetup::new();
    out.amplitude(tomlget_or!(
        cfg,
        "ramp",
        "amplitude_volts",
        as_float,
        f32,
        1.0
    ));
    out.piezo_settle_time_ms(tomlget_or!(
        cfg,
        "ramp",
        "piezo_settle_time_ms",
        as_float,
        f32,
        12.0
    ));
    out.piezo_scale_factor(tomlget_or!(
        cfg,
        "ramp",
        "piezo_scale_factor",
        as_float,
        f32,
        2000.0
    ));
    let dec = tomlget_or!(cfg, "ramp", "decimation_factor", as_integer, u32, 16);
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
        eprintln!("Decimation factor specified in config file as {}. Valid decimation factors are 1, 2, 4, 8, or any value between 16 and 65536. Proceeding with decimation factor of {}", dec, dec_factor);
    }
    out.set_decimation(dec_factor);

    out.set_symmetry(
        tomlget_or!(cfg, "ramp", "symmetry_factor", as_float, f32, 0.8).clamp(0.01, 0.99),
    );

    Ok(out)
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
        let buffer_size_exponent = length.checked_ilog2().unwrap_or(0) as usize;
        if buffer_size_exponent != length as usize {
            eprintln!(
                "WARN: config explicit log length parameter {} rounded down to 2^{} = {}.",
                length,
                buffer_size_exponent,
                1 << buffer_size_exponent,
            );
        }
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

pub fn ref_laser_from_config(cfg: &toml::Value) -> Result<ReferenceLaser, String> {
    let hostname = gethostname()
        .into_string()
        .map_err(|_| "failed to get hostname")?;
    let hostname = hostname.as_str();
    let is_master = tomlget_or!(cfg, hostname, "is_master", as_bool, false);

    let buffer_size_exponent = buff_size_exponent(cfg);

    let mut out =
        ReferenceLaser::new(buffer_size_exponent).ok_or("failed to instantiate laser struct")?;
    out.set_wavelength(
        tomlget!(cfg, "ref_laser", "wavelength_nm", as_float, f32),
        tomlget_or!(cfg, "ramp", "piezo_scale_factor", as_float, f32, 2000.0),
        tomlget_or!(cfg, "ramp", "amplitude_volts", as_float, f32, 1.0),
    );
    match tomlget!(cfg, hostname, "ref_input_channel", as_str) {
        // match cfg.get(hostname)?.get("ref_input_channel")?.as_str()? {
        "CH_1" | "CH_A" => out.input_channel = core::RPCoreChannel::CH_1,
        "CH_2" | "CH_B" => out.input_channel = core::RPCoreChannel::CH_2,
        _ => {
            return Err("No valid input channel for reference laser found".into());
        }
    };
    if is_master {
        match tomlget!(cfg, hostname, "ref_output_channel", as_str) {
            // match cfg.get(hostname)?.get("ref_output_channel")?.as_str()? {
            "CH_1" | "CH_A" => out.output_channel = Some(core::RPCoreChannel::CH_1),
            "CH_2" | "CH_B" => out.output_channel = Some(core::RPCoreChannel::CH_2),
            _ => {
                return Err("No valid output channel for reference laser found".into());
            }
        };
    } else {
        out.output_channel = None;
    }

    // fill in ``guess'' fit coefficients for the lasers
    out.fit_coefficients = [0.0, out.fringe_freq(), 0.0, 0.0, 1000.0];
    Ok(out)
}
pub fn slave_laser_from_config(cfg: &toml::Value) -> Result<SlaveLaser, String> {
    let hostname = gethostname()
        .into_string()
        .map_err(|_| "failed to get hostname")?;
    let hostname = hostname.as_str();
    let slave_laser_name = tomlget!(cfg, hostname, "slave_laser", as_str);
    let buffer_size_exponent = buff_size_exponent(cfg);

    let mut out =
        SlaveLaser::new(buffer_size_exponent).ok_or("failed to instantiate laser struct")?;
    out.set_wavelength(
        tomlget!(cfg, slave_laser_name, "wavelength_nm", as_float, f32),
        tomlget_or!(cfg, "ramp", "piezo_scale_factor", as_float, f32, 2000.0),
        tomlget_or!(cfg, "ramp", "amplitude_volts", as_float, f32, 1.0),
    );
    match tomlget!(cfg, hostname, "slave_input_channel", as_str) {
        // match cfg.get(hostname)?.get("ref_input_channel")?.as_str()? {
        "CH_1" | "CH_A" => out.input_channel = core::RPCoreChannel::CH_1,
        "CH_2" | "CH_B" => out.input_channel = core::RPCoreChannel::CH_2,
        _ => {
            return Err("No valid input channel for reference laser found".to_string());
        }
    };
    match tomlget!(cfg, hostname, "slave_output_channel", as_str) {
        // match cfg.get(hostname)?.get("ref_output_channel")?.as_str()? {
        "CH_1" | "CH_A" => out.output_channel = Some(core::RPCoreChannel::CH_1),
        "CH_2" | "CH_B" => out.output_channel = Some(core::RPCoreChannel::CH_2),
        _ => {
            return Err("No valid output channel for reference laser found".to_string());
        }
    };

    // fill in ``guess'' fit coefficients for the lasers
    out.fit_coefficients = [0.0, out.fringe_freq(), 0.0, 0.0, 1000.0];
    Ok(out)
}

pub fn ref_lock_from_config(cfg: &toml::Value) -> Result<Servo, String> {
    // let hostname = gethostname()
    //     .into_string()
    //     .map_err(|_| "failed to get hostname")?;
    // let hostname = hostname.as_str();
    // let is_master = tomlget_or!(cfg, hostname, "is_master", as_bool, false);
    let mut out = Servo::default();
    out.gain_P = tomlget_or!(cfg, "ref_laser", "gain_p", as_float, f32, 0.2);
    out.gain_I = tomlget_or!(cfg, "ref_laser", "gain_i", as_float, f32, 0.1);
    out.gain_D = tomlget_or!(cfg, "ref_laser", "gain_d", as_float, f32, 0.0);
    out.set_alpha_I(tomlget_or!(
        cfg,
        "ref_laser",
        "integral_decay_rate",
        as_float,
        f32,
        0.85
    ));
    out.max_feedback_step_size = tomlget_or!(
        cfg,
        "ref_laser",
        "feedback_max_step_size_v",
        as_float,
        f32,
        1.0
    );
    let max_err_tolerance_MHz = tomlget_or!(
        cfg,
        "ref_laser",
        "max_err_tolerance_MHz",
        as_float,
        f32,
        f64::INFINITY
    );
    out.err_max_tolerance = max_err_tolerance_MHz * 2.0 * PI
        / tomlget!(cfg, "general", "interferometer_FSR_MHz", as_float, f32);
    Ok(out)
}
pub fn slave_lock_from_config(cfg: &toml::Value) -> Result<Servo, String> {
    let hostname = gethostname()
        .into_string()
        .map_err(|_| "failed to get hostname")?;
    let hostname = hostname.as_str();
    let slave_laser_name = tomlget!(cfg, hostname, "slave_laser", as_str);
    let mut out = Servo::default();
    out.gain_P = tomlget_or!(cfg, slave_laser_name, "gain_p", as_float, f32, 0.001);
    out.gain_I = tomlget_or!(cfg, slave_laser_name, "gain_i", as_float, f32, 0.003);
    out.gain_D = tomlget_or!(cfg, slave_laser_name, "gain_d", as_float, f32, 0.0);
    out.set_alpha_I(tomlget_or!(
        cfg,
        slave_laser_name,
        "integral_decay_rate",
        as_float,
        f32,
        0.85
    ));
    out.max_feedback_step_size = tomlget_or!(
        cfg,
        slave_laser_name,
        "feedback_max_step_size_v",
        as_float,
        f32,
        0.01
    );
    let max_err_tolerance_MHz = tomlget_or!(
        cfg,
        slave_laser_name,
        "max_err_tolerance_MHz",
        as_float,
        f32,
        10.0
    );
    out.err_max_tolerance = max_err_tolerance_MHz * 2.0 * PI
        / tomlget!(cfg, "general", "interferometer_FSR_MHz", as_float, f32);
    out.default_output_voltage = cfg
        .get(slave_laser_name)
        .and_then(|x| x.get("default_output_voltage"))
        .and_then(|x| x.as_float())
        .map(|x| x as f32);
    Ok(out)
}

pub fn multifit_from_config(cfg: &toml::Value) -> Result<FitSetup, String> {
    let num_points = (16384
        - tomlget_or!(cfg, "multifit", "samples_skip_start", as_integer, u32, 6000)
        - tomlget_or!(cfg, "multifit", "samples_skip_end", as_integer, u32, 0)
        + tomlget_or!(cfg, "multifit", "skip_rate", as_integer, u32, 40)
        - 1)
        / tomlget_or!(cfg, "multifit", "skip_rate", as_integer, u32, 40);
    let stride = tomlget_or!(cfg, "multifit", "skip_rate", as_integer, u32, 40);
    let iters = tomlget_opt!(cfg, "multifit", "max_iterations", as_integer, u32);
    Ok(FitSetup::new(num_points)
        .stride(stride)
        .opt_max_iterations(iters)
        .opt_xtol(tomlget_opt!(cfg, "multifit", "xtol", as_float, f32))
        .opt_gtol(tomlget_opt!(cfg, "multifit", "gtol", as_float, f32))
        .opt_ftol(tomlget_opt!(cfg, "multifit", "ftol", as_float, f32))
        .opt_max_av_ratio(tomlget_opt!(cfg, "multifit", "max_av_ratio", as_float, f32))
        .opt_low_contrast_threshold((|| {
            tomlget_opt!(
                cfg,
                gethostname()
                    .to_str()
                    .and_then(|h| tomlget_opt!(cfg, h, "slave_laser", as_str))?,
                "low_contrast_threshold",
                as_float,
                f32
            )
        })())
        .init()
        .ok_or("failed to instantate multifit setup")?)
}

pub fn interferometer_from_config(cfg: &toml::Value) -> Result<Interferometer, String> {
    let mut out = Interferometer::new().ok_or("failed to instantiate interferometer struct")?;

    out.ramp_setup = ramp_from_config(cfg)?;
    out.ref_laser = ref_laser_from_config(cfg)?;
    out.slave_laser = slave_laser_from_config(cfg)?;
    out.ref_lock = ref_lock_from_config(cfg)?;
    out.slave_lock = slave_lock_from_config(cfg)?;
    out.fit_setup_ref = multifit_from_config(cfg)?;
    out.fit_setup_slave = multifit_from_config(cfg)?;
    Ok(out)
}
