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
use std::thread::spawn;
use std::time::Instant;
use std::{env, thread, time};

// use rand::distributions::{Distribution, Uniform};

use chrono::Local;

use librp_sys::dpin;
use librp_sys::generator::{DCChannel, PulseChannel};
use librp_sys::Pitaya;
use librp_sys::{core, oscilloscope};

use rusterf::configs;
use rusterf::multifit;

// mod lib;
// use lib::laser::Laser;

macro_rules! data_ch {
    ($laser:expr, $pit:ident) => {
        match $laser.input_channel {
            core::Channel::CH_1 => &$pit.scope.chA_buff_float,
            core::Channel::CH_2 => &$pit.scope.chB_buff_float,
        }
    };
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::cast_possible_truncation)]
#[async_std::main]
async fn main() {
    let mut pit = Pitaya::init().expect("Failed to intialize the Red Pitaya!");
    pit.gen
        .reset()
        .expect("Failed to reset rp function generator");

    let path_base = env::current_exe().expect("Failed to get the path to this program");
    println!(
        "Reading config file {}",
        path_base.with_file_name("config.toml").display()
    );

    let cfg_text = read_to_string(path_base.with_file_name("config.toml"))
        .expect("Failed to open config file!");
    let cfg = toml::from_str(&cfg_text).expect("Failed to parse config file");
    let mut interf = match configs::interferometer_from_config(&cfg) {
        Ok(x) => x,
        Err(e) => panic!("[{}] error [{}] in reading config file", Local::now(), e),
    };
    let mut interf_comms = match configs::comms_from_config(&cfg).await {
        Ok(x) => x,
        Err(e) => panic!("[{}] error [{}] in reading config file", Local::now(), e),
    };

    let DO_DEBUG_LOGGING;
    let DEBUG_LOG_FREQ_LOG;
    if let Some(toml::Value::Integer(freq)) = cfg
        .get("general")
        .expect("already read in interferometer_from_config")
        .get("debug_list_freq_cycles")
    {
        DO_DEBUG_LOGGING = true;
        DEBUG_LOG_FREQ_LOG = configs::floor_exp(*freq as u32);
    } else {
        DO_DEBUG_LOGGING = false;
        DEBUG_LOG_FREQ_LOG = 0;
    }

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
    if interf.is_master() {
        pit.dpin
            .set_state(trigger_pin, dpin::PinState::Low)
            .expect("API call should succeed");
    }

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
    interf
        .ramp_setup
        .apply(&mut pit.scope, ramp_ch.as_mut(), &mut slave_out_ch)
        .expect("failed to apply ramp settings");

    pit.scope
        .start_acquisition()
        .expect("Failed to start data acquisition");
    let _ = pit
        .scope
        .set_trigger_source(oscilloscope::TrigSrc::ExtRising);
    thread::sleep(time::Duration::from_millis(50));

    let mut triggered: Instant;
    let mut fit_started: Instant;
    let mut total_fitting_time_us: u32 = 0;
    let mut total_err_ref: f32 = 0.0;
    let mut variance_ref: f32 = 0.0;
    let mut total_err_slave: f32 = 0.0;
    let mut variance_slave: f32 = 0.0;
    let mut iterations_ref = 0;
    let mut iterations_slave = 0;

    let rayon_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();

    let last_ref_result: Option<multifit::FitResult> = None;
    let last_slave_result: Option<multifit::FitResult> = None;

    println!("fitting with n = {:?}", interf.fit_setup_ref.num_points);
    println!("Entering main loop...");
    loop {
        interf.cycle_counter += 1;

        if interf.is_master() {
            interf.ref_lock.enable();
        }

        if interf.is_master() {
            loop {
                if let Ok(dpin::PinState::Low) = pit.dpin.get_state(ready_to_acquire_pin) {
                    pit.dpin.set_state(trigger_pin, dpin::PinState::High);
                    break;
                };
            }
        } else {
            let _ = pit
                .dpin
                .set_direction(ready_to_acquire_pin, dpin::PinDirection::In);
        }

        loop {
            if let Ok(oscilloscope::TrigState::Triggered) = pit.scope.get_trigger_state() {
                triggered = Instant::now();
                break;
            };
        }

        if !interf.is_master() {
            let _ = pit
                .dpin
                .set_direction(ready_to_acquire_pin, dpin::PinDirection::Out);
            let _ = pit
                .dpin
                .set_state(ready_to_acquire_pin, dpin::PinState::High);
        }

        if interf_comms.should_publish_logs(interf.cycle_counter) {
            match interf_comms.publish_logs(&mut interf).await {
                Ok(()) => {}
                Err(x) => {
                    eprintln!("[{}] Failed to publish logs: error [{}]", Local::now(), x);
                }
            }
        }
        while let Some(request) = interf_comms.handle_socket_request(&mut interf).await {
            println!("[{}] Handled socket request <{}>", Local::now(), request);
        }

        if DO_DEBUG_LOGGING && interf.cycle_counter & ((1 << DEBUG_LOG_FREQ_LOG) - 1) == 0 {
            let denom = 2.0_f32.powi(DEBUG_LOG_FREQ_LOG.into());
            println!(
                "[{}] average fitting time {} us",
                Local::now(),
                total_fitting_time_us >> 9
            );
            total_fitting_time_us = 0;
            println!(
                "\taverage iterations per fit cycle: [ref: {:.2}, slave: {:.2}]",
                iterations_ref as f32 / denom,
                iterations_slave as f32 / denom,
            );
            iterations_ref = 0;
            iterations_slave = 0;
            println!(
                "\taverage phase error (rad): [ref: {:.2}, slave: {:.2}]",
                total_err_ref / denom,
                total_err_slave / denom,
            );
            println!(
                "\tRMS phase error (rad): [ref: {:.4}, slave: {:.4}]",
                (variance_ref / denom).sqrt(),
                (variance_slave / denom).sqrt(),
            );
            total_err_ref = 0.0;
            variance_ref = 0.0;
            total_err_slave = 0.0;
            variance_slave = 0.0;
        }

        // if the last fit got a suspicious result, we should reset our ''guess'' parameters
        // to try to avoid getting stuck fitting to a bad mode. Also just reset the guess
        // occasionally just in case.
        let reset_timer = interf.cycle_counter & ((1 << 12) - 1) == 0;
        if reset_timer || last_ref_result.as_ref().map_or(false, |r| r.low_contrast) {
            interf.ref_laser.fit_coefficients = [0.0, interf.ref_laser.fringe_freq(), 0.0, 1000.0];
        }
        if reset_timer || last_slave_result.as_ref().map_or(false, |r| r.low_contrast) {
            interf.slave_laser.fit_coefficients =
                [0.0, interf.slave_laser.fringe_freq(), 0.0, 1000.0];
        }

        loop {
            if triggered.elapsed().as_nanos() > interf.ramp_setup.rise_time_ns() {
                break;
            };
        }
        let _ = pit.scope.update_scope_data_both();
        if interf.is_master() {
            let _ = pit.dpin.set_state(trigger_pin, dpin::PinState::Low);
        }

        fit_started = Instant::now();
        // Can also accomplish this with a 'scoped thread'
        let (ref_result, slave_result) = rayon_pool.join(
            || {
                interf.fit_setup_ref.fit(
                    data_ch!(interf.ref_laser, pit).as_slice(),
                    interf.ref_laser.fit_coefficients,
                )
            },
            || {
                interf.fit_setup_slave.fit(
                    data_ch!(interf.slave_laser, pit).as_slice(),
                    interf.slave_laser.fit_coefficients,
                )
            },
        );
        total_fitting_time_us += fit_started.elapsed().as_micros() as u32;
        iterations_ref += ref_result.n_iterations;
        iterations_slave += slave_result.n_iterations;

        interf.ref_laser.fit_coefficients = ref_result.params;
        interf.slave_laser.fit_coefficients = slave_result.params;

        let ref_error =
            multifit::wrapped_angle_difference(ref_result.params[2], interf.ref_lock.setpoint());
        let slave_error = multifit::wrapped_angle_difference(
            slave_result.params[2]
                - interf.ref_lock.last_error() * interf.ref_laser.wavelength_nm()
                    / interf.slave_laser.wavelength_nm(),
            interf.slave_lock.setpoint(),
        );
        total_err_ref += ref_error;
        variance_ref += ref_error * ref_error;
        total_err_slave += slave_error;
        variance_slave += slave_error * slave_error;

        let ref_adjustment = interf.ref_lock.do_pid(ref_error);
        let slave_adjustment = interf.slave_lock.do_pid(slave_error);

        if ramp_ch.is_some() {
            let _ = ramp_ch.as_mut().unwrap().increment_offset(ref_adjustment);
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

        let last_ref_result = Some(ref_result);
        let last_slave_result = Some(slave_result);

        if interf_comms.should_publish_logs(interf.cycle_counter + 4) {
            // Ideally we'd always send the most recent waveform, but we handle communications
            // while the scope is acquiring, i.e. while the most recent waveform is being written in
            // memory. Instead, we have to copy the waveform ahead of time, but this large of a
            // memory operation can take a few milliseconds, which slightly distorts the next
            // waveform acquired. So we copy the waveform a few cycles ahead of our next
            // communications event, so in effect when we publish a 'most recent waveform', it's
            // actually a few cycles out of date.
            let _ = match interf.ref_laser.input_channel {
                core::Channel::CH_1 => pit.scope.write_raw_waveform(
                    &mut interf.last_waveform_ref,
                    &mut interf.last_waveform_slave,
                ),
                core::Channel::CH_2 => pit.scope.write_raw_waveform(
                    &mut interf.last_waveform_slave,
                    &mut interf.last_waveform_ref,
                ),
            };
        }

        let _ = pit.scope.start_acquisition();
        let _ = pit
            .scope
            .set_trigger_source(oscilloscope::TrigSrc::ExtRising);

        loop {
            if triggered.elapsed().as_micros() as u64
                > (interf.ramp_setup.ramp_period_us() + interf.ramp_setup.piezo_settle_time_us())
            {
                // println!(
                //     "elapsed time: {} us vs ramp + settle {} us",
                //     triggered.elapsed().as_micros() as u64,
                //     interf.ramp_setup.ramp_period_us() + interf.ramp_setup.piezo_settle_time_us()
                // );
                break;
            }
        }
    }
}
