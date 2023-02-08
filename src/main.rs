#![warn(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(clippy::cast_precision_loss)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::f32::consts::PI;
use std::fs::read_to_string;
use std::str::FromStr;
use std::time::Instant;
use std::{env, thread, time};

use rand::distributions::{Distribution, Uniform};

use chrono::Local;
// extern crate toml;
extern crate zmq;
use rayon::join;

use librp_sys::dpin;
use librp_sys::generator::{DCChannel, PulseChannel};
use librp_sys::oscilloscope::Oscilloscope;
use librp_sys::Pitaya;
use librp_sys::{core, oscilloscope};

use self::lib::circle_buffer::CircleBuffer2n;

pub mod lib;
mod multifit;

use crate::lib::configs;
use crate::lib::interferometer::Interferometer;
// use lib::laser::Laser;
use crate::multifit::{sinusoid, wrapped_angle_difference, FitSetup};

macro_rules! data_ch {
    ($laser:expr, $pit:ident) => {
        match $laser.input_channel {
            core::Channel::CH_1 => &$pit.scope.chA_buff_float,
            core::Channel::CH_2 => &$pit.scope.chB_buff_float,
        }
    };
}

#[allow(clippy::too_many_lines)]
fn main() {
    let mut pit = Pitaya::init().expect("Failed to intialize the Red Pitaya!");

    let ctx = zmq::Context::new();

    let path_base = env::current_exe().expect("Failed to get the path to this program");
    println!(
        "Reading config file {}",
        path_base.with_file_name("config.toml").display()
    );

    let cfg_text = read_to_string(path_base.with_file_name("config.toml"))
        .expect("Failed to open config file!");
    let cfg = toml::from_str(&cfg_text).expect("Failed to parse config file");
    let mut interf = configs::interferometer_from_config(&cfg_text)
        .expect("Failed to construct interferometer object from config file");
    let mut interf_comms = configs::comms_from_config(&cfg_text, &ctx)
        .expect("Failed to construct sockets from config file");

    if interf.is_master() {
        println!("Designated as MASTER RP; controlling interferometer voltage ramp");
    }

    configs::generator_from_config(&cfg, &mut pit.gen)
        .expect("Failed to set up waveform generator from config file");
    configs::scope_from_config(&cfg, &mut pit.scope)
        .expect("Failed to set up scope from config file");
    configs::dpin_from_config(&cfg, &mut pit.dpin)
        .expect("Failed to set up Digital IO pins from config file");
    let ready_to_acquire_pin = configs::dpin_get_ready_pin(&cfg).expect("already set up pins");
    let trigger_pin = configs::dpin_get_trigger_pin(&cfg).expect("already set up pins");

    let mut ramp_ch;
    let mut slave_out_ch;
    match interf.ref_laser.output_channel {
        Some(core::Channel::CH_1) => {
            ramp_ch = Some(
                PulseChannel::init(&mut pit.gen.ch_a, vec![0.0; 16], 1.0)
                    .expect("failed to initialize pulse_channel!"),
            );
            slave_out_ch =
                DCChannel::init(&mut pit.gen.ch_b).expect("failed to initialize dc_channel!");
        }

        Some(core::Channel::CH_2) => {
            ramp_ch = Some(
                PulseChannel::init(&mut pit.gen.ch_b, vec![0.0; 16], 1.0)
                    .expect("failed to initialize pulse_channel!"),
            );
            slave_out_ch =
                DCChannel::init(&mut pit.gen.ch_a).expect("failed to initialize dc_channel!");
        }
        None => {
            ramp_ch = None;
            slave_out_ch =
                match interf
                    .slave_laser
                    .output_channel
                    .expect("interferometer_from_config already set up slave output channel")
                {
                    core::Channel::CH_1 => DCChannel::init(&mut pit.gen.ch_a)
                        .expect("failed to initialize dc_channel!"),
                    core::Channel::CH_2 => DCChannel::init(&mut pit.gen.ch_b)
                        .expect("failed to initialize dc_channel!"),
                };
        }
    };

    pit.scope
        .start_acquisition()
        .expect("Failed to start data acquisition");
    thread::sleep(time::Duration::from_millis(50));

    let mut triggered: Instant;
    let mut fit_started: Instant;
    let mut total_fitting_time_us: u32 = 0;

    println!("Entering main loop...");
    loop {
        interf.cycle_counter += 1;

        if interf.is_master() {
            loop {
                if let Ok(dpin::PinState::Low) = pit.dpin.get_state(ready_to_acquire_pin) {
                    break;
                };
            }
        } else {
            pit.dpin
                .set_direction(ready_to_acquire_pin, dpin::PinDirection::In);
        }

        loop {
            if let Ok(oscilloscope::TrigState::Triggered) = pit.scope.get_trigger_state() {
                triggered = Instant::now();
                break;
            };
        }

        if !interf.is_master() {
            pit.dpin
                .set_state(ready_to_acquire_pin, dpin::PinState::High);
        }

        //TODO: Update waveforms for publishing
        if interf_comms.should_publish_logs(interf.cycle_counter) {
            match interf_comms.publish_logs(&mut interf) {
                Ok(()) => {}
                Err(x) => {
                    eprintln!("[{}] Failed to publish logs: error [{}]", Local::now(), x);
                }
            }
        }
        while let Some(request) = interf_comms.handle_socket_request(&mut interf) {
            println!("[{}] Handled socket request <{}>", Local::now(), request);
        }

        //TODO: Debug loggin/printing

        loop {
            if triggered.elapsed().as_nanos() > interf.ramp_setup.rise_time_ns() {
                break;
            };
        }
        pit.scope.update_scope_data_both();
        if interf.is_master() {
            pit.dpin.set_state(trigger_pin, dpin::PinState::High);
        }

        fit_started = Instant::now();
        // let (ref_result, slave_result) = rayon::join(
        //     || {
        //         interf.fit_setup_ref.fit(
        //             data_ch!(interf.ref_laser, pit).as_slice(),
        //             interf.ref_laser.fit_coefficients,
        //         )
        //     },
        //     || {
        //         interf.fit_setup_slave.fit(
        //             data_ch!(interf.slave_laser, pit).as_slice(),
        //             interf.slave_laser.fit_coefficients,
        //         )
        //     },
        // );
        let ref_result = interf.fit_setup_ref.fit(
            data_ch!(interf.ref_laser, pit).as_slice(),
            interf.ref_laser.fit_coefficients,
        );

        let slave_result = interf.fit_setup_slave.fit(
            data_ch!(interf.slave_laser, pit).as_slice(),
            interf.slave_laser.fit_coefficients,
        );
        total_fitting_time_us += fit_started.elapsed().as_micros() as u32;
        // println!(
        //     "[{}] fitting time {} us",
        //     Local::now(),
        //     total_fitting_time_us
        // );
        if interf.cycle_counter & ((1 << 9) - 1) == 0 {
            println!(
                "[{}] average fitting time {} us",
                Local::now(),
                total_fitting_time_us >> 9
            );
            total_fitting_time_us = 0;
        }

        let ref_error =
            multifit::wrapped_angle_difference(ref_result.params[2], interf.ref_lock.setpoint());
        let slave_error = multifit::wrapped_angle_difference(
            slave_result.params[2]
                - interf.ref_lock.last_error() * interf.ref_laser.wavelength_nm()
                    / interf.slave_laser.wavelength_nm(),
            interf.slave_lock.setpoint(),
        );
        let ref_adjustment = interf.ref_lock.do_pid(ref_error);
        let slave_adjustment = interf.slave_lock.do_pid(slave_error);

        if ramp_ch.is_some() {
            ramp_ch.as_mut().unwrap().increment_offset(ref_adjustment);
        }
        slave_out_ch.increment_offset(slave_adjustment);

        interf.ref_laser.phase_log.push(ref_error);
        interf
            .ref_laser
            .feedback_log
            .push(ramp_ch.as_ref().map_or(0.0, PulseChannel::offset_v));
        interf.slave_laser.phase_log.push(slave_error);
        interf
            .slave_laser
            .feedback_log
            .push(slave_out_ch.offset_v());

        pit.scope.start_acquisition();
        pit.scope
            .set_trigger_source(oscilloscope::TrigSrc::ExtRising);

        loop {
            if triggered.elapsed().as_micros() as i64
                > (interf.ramp_setup.ramp_period_us() + interf.ramp_setup.piezo_settle_time_us())
            {
                break;
            }
        }
    }
}
