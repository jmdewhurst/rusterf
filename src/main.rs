#![warn(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(clippy::cast_precision_loss)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::f32::consts::PI;
use std::fs::read_to_string;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::str::FromStr;
use std::thread::spawn;
use std::time::Instant;
use std::{env, thread, time};

use async_std::task::block_on;
use chrono::Local;

use librp_sys::dpin;
use librp_sys::Pitaya;
use librp_sys::{core, oscilloscope};

use rusterf::configs;
use rusterf::interferometer::State;
use rusterf::multifit;
use rusterf::util::find_file;

macro_rules! data_ch {
    ($laser:expr, $pit:ident) => {
        match $laser.input_channel {
            core::RPCoreChannel::CH_1 => &$pit.scope.chA_buff_float,
            core::RPCoreChannel::CH_2 => &$pit.scope.chB_buff_float,
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

    let cfg_file = find_file(Path::new("config.toml")).expect("Failed to find configuration file!");
    println!("Reading from config file {cfg_file:?}");

    let cfg_text = read_to_string(cfg_file).expect("Failed to open config file!");
    let cfg = toml::from_str(&cfg_text).expect("Failed to parse config file");
    let mut interf = match configs::interferometer_from_config(&cfg) {
        Ok(x) => x,
        Err(e) => panic!("[{}] error [{}] in reading config file", Local::now(), e),
    };
    let mut interf_comms = match configs::comms_from_config(&cfg).await {
        Ok(x) => x,
        Err(e) => panic!("[{}] error [{}] in reading config file", Local::now(), e),
    };
    interf
        .stats
        .set_averaging(1 << interf_comms.logs_publish_frequency_exponent());

    let DEBUG_LOG_FREQ_LOG = (|| {
        cfg.get("general")?
            .get("debug_list_freq_cycles")?
            .as_integer()?
            .checked_ilog2()
    })()
    .unwrap_or_default();

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

    let ramp_ch_raw;
    let slave_out_ch_raw;
    match (
        interf.ref_laser.output_channel,
        interf.slave_laser.output_channel,
    ) {
        (Some(core::RPCoreChannel::CH_1), Some(core::RPCoreChannel::CH_2)) => {
            ramp_ch_raw = Some(&mut pit.gen.ch_a);
            slave_out_ch_raw = &mut pit.gen.ch_b;
        }
        (Some(core::RPCoreChannel::CH_2), Some(core::RPCoreChannel::CH_1)) => {
            ramp_ch_raw = Some(&mut pit.gen.ch_b);
            slave_out_ch_raw = &mut pit.gen.ch_a;
        }
        (None, Some(core::RPCoreChannel::CH_1)) => {
            ramp_ch_raw = None;
            slave_out_ch_raw = &mut pit.gen.ch_a;
        }
        (None, Some(core::RPCoreChannel::CH_2)) => {
            ramp_ch_raw = None;
            slave_out_ch_raw = &mut pit.gen.ch_b;
        }
        (_, None) => {
            panic!("Fatal: No slave laser output channel found. Check configuration file.");
        }
        (x, y) => {
            panic!("Fatal: Failed to set reference laser output to channel {x:?} and slave laser output to channel {y:?}. Does the configuration file list both lasers on the same output channel?");
        }
    };
    let (mut ramp_ch, mut slave_out_ch) = interf
        .ramp_setup
        .slave_default_offset_v(interf.slave_servo.default_output_voltage)
        .apply(&mut pit.scope, ramp_ch_raw, slave_out_ch_raw)
        .expect("failed to apply ramp settings");

    let wavelength_ratio = interf.ref_laser.wavelength_nm() / interf.slave_laser.wavelength_nm();

    pit.scope
        .start_acquisition()
        .expect("Failed to start data acquisition");
    let _ = pit
        .scope
        .set_trigger_source(oscilloscope::TrigSrc::ExtRising);
    thread::sleep(time::Duration::from_millis(50));

    find_file(Path::new("swap.toml")).map(|file| {
        println!("found swap file `{file:?}` -- attempting to recover");
        let contents = read_to_string(&file).ok()?;

        match toml::from_str::<State>(&contents)
            .map(|state| interf.apply_state(state, &mut slave_out_ch))
        {
            Ok(Ok(())) => Some(()),
            Ok(Err(err)) => {
                println!("swap file failed with error {err:?} -- attempting to delete");
                std::fs::remove_file(&file).ok()
            }
            Err(err) => {
                println!("swap file failed with error {err:?} -- attempting to delete");
                std::fs::remove_file(&file).ok()
            }
        }
    });

    let mut triggered: Instant;
    let mut fit_started: Instant;

    let rayon_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .unwrap();

    let mut last_ref_result: multifit::FitResult = Default::default();
    let mut last_slave_result: multifit::FitResult = Default::default();

    println!("fitting with n = {:?}", interf.fit_setup_ref.num_points);
    println!("Entering main loop...");
    loop {
        interf.cycle_counter += 1;

        if interf.is_master() {
            loop {
                if let Ok(dpin::PinState::Low) = pit.dpin.get_state(ready_to_acquire_pin) {
                    let _ = pit.dpin.set_state(trigger_pin, dpin::PinState::High);
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

        if DEBUG_LOG_FREQ_LOG != 0 && interf.cycle_counter & ((1 << DEBUG_LOG_FREQ_LOG) - 1) == 0 {
            let stats = interf.stats.evaluate();
            let denom = 2.0_f32.powi(DEBUG_LOG_FREQ_LOG as i32);
            println!(
                "\taverage iterations per fit cycle: [ref: {:.2}, slave: {:.2}]",
                stats.avg_iterations_ref, stats.avg_iterations_slave,
            );
            println!(
                "\taverage phase error (rad): [ref: {:.2}, slave: {:.2}]",
                stats.avg_err_ref, stats.avg_err_slave,
            );
            println!(
                "\tRMS phase error (rad): [ref: {:.4}, slave: {:.4}]",
                stats.variance_ref, stats.variance_slave,
            );
            println!("ref fit {:?}", last_ref_result.params);
            println!("\tchisq/dof: ref {},", last_ref_result.reduced_chisq());
            println!("slave fit {:?}", last_slave_result.params);
            println!("slave chisq/dof {}", last_slave_result.reduced_chisq());
            println!(
                "ref posn {:?} slave posn {:?}, slave_last_err {:?}, slave total error {:?}",
                interf.ref_position_lock.setpoint(),
                interf.slave_position_lock.setpoint(),
                interf.slave_position_lock.last_error(),
                interf.slave_servo.last_error()
            );
        }

        if interf_comms.should_publish_logs(interf.cycle_counter) {
            match catch_unwind(AssertUnwindSafe(|| {
                block_on(interf_comms.publish_logs(
                    &mut interf,
                    last_ref_result.reduced_chisq(),
                    last_slave_result.reduced_chisq(),
                ))
            })) {
                Ok(Ok(())) => {}
                Ok(Err(x)) => {
                    eprintln!("[{}] Failed to publish logs: error [{}]", Local::now(), x);
                }
                Err(_) => {
                    eprintln!("[{}] Panic in publish_logs", Local::now());
                    let _ = interf_comms.unbind_sockets().await;
                    let _ = interf_comms
                        .bind_sockets(interf_comms.logs_port(), interf_comms.command_port())
                        .await;
                }
            }
            interf.stats.reset();
        }

        if interf.do_swap_file && interf.cycle_counter % 256 == 0 {
            if let Ok(Ok(s)) = interf.state().map(|x| toml::to_string(&x)) {
                let _ = std::fs::write("swap.toml", &s);
            }
        }

        // if the last fit got a suspicious result, we should reset our ''guess'' parameters
        // to try to avoid getting stuck fitting to a bad mode. Also just reset the guess
        // occasionally just in case.
        let reset_timer = interf.cycle_counter & ((1 << 14) - 1) == 0;
        if reset_timer
            || last_ref_result.low_contrast
            || last_ref_result.invalid_params
            || last_ref_result.chisq > (5000 * last_slave_result.dof) as f32
        {
            interf.ref_laser.fit_coefficients =
                [0.0, interf.ref_laser.fringe_freq(), 0.0, 0.0, 1000.0];
        }
        if reset_timer
            || last_slave_result.low_contrast
            || last_slave_result.invalid_params
            || last_slave_result.chisq > (5000 * last_slave_result.dof) as f32
        {
            interf.slave_laser.fit_coefficients[0] = 0.0;
            interf.slave_laser.fit_coefficients[1] = interf.slave_laser.fringe_freq();
            interf.slave_laser.fit_coefficients[3] = interf.ref_laser.fit_coefficients[3]
                * interf.slave_laser.fringe_freq()
                / interf.ref_laser.fringe_freq();
            // interf.slave_laser.fit_coefficients =
            //     [0.0, interf.slave_laser.fringe_freq(), 0.0, interf.ref_laser.fit_coefficents[3] * interf.slave_laser.fringe_freq() / interf.ref_laser.fringe_freq(), 1000.0];
        }

        loop {
            if triggered.elapsed().as_nanos() > interf.ramp_setup.rise_time_ns() {
                break;
            } else if let Some(request) = interf_comms
                .handle_socket_request(&mut interf, ramp_ch.as_mut(), &mut slave_out_ch)
                .await
            {
                println!("[{}] Handled socket request <{}>", Local::now(), request);
            }
        }
        let _ = pit.scope.update_scope_data_both();
        if interf.is_master() {
            let _ = pit.dpin.set_state(trigger_pin, dpin::PinState::Low);
        }

        fit_started = Instant::now();
        // Can also accomplish this with a 'scoped thread'
        let (ref_result, slave_result) = rayon_pool.join(
            || {
                interf.fit_setup_ref.fit_five_parameter(
                    data_ch!(interf.ref_laser, pit),
                    interf.ref_laser.fit_coefficients,
                )
            },
            || {
                interf.fit_setup_slave.fit_five_parameter(
                    data_ch!(interf.slave_laser, pit),
                    interf.slave_laser.fit_coefficients,
                )
            },
        );

        let ref_error = multifit::wrapped_angle_difference(
            ref_result.params[2],
            interf.ref_position_lock.setpoint(),
        );
        let slave_novel_error = multifit::wrapped_angle_difference(
            slave_result.params[2] - ref_error * wavelength_ratio,
            interf.slave_position_lock.setpoint()
                + interf.ref_position_lock.setpoint() * wavelength_ratio,
        );
        let slave_absolute_error = slave_novel_error + interf.slave_position_lock.setpoint()
            - interf.slave_servo.setpoint();
        interf
            .stats
            .new_time_us(fit_started.elapsed().as_micros() as u32)
            .new_errs(ref_error, slave_absolute_error)
            .new_iterations(
                ref_result.n_iterations as u32,
                slave_result.n_iterations as u32,
            );
        interf.ref_laser.fit_coefficients = ref_result.params;
        interf.slave_laser.fit_coefficients = slave_result.params;
        interf.ref_laser.fit_coefficient_errs = ref_result.param_errs;
        interf.slave_laser.fit_coefficient_errs = slave_result.param_errs;

        // we don't actually servo on the reference laser, just use the pid loop to decide where
        // the zero-length point is in the interferometer, and adjust the slave laser accordingly
        // Only do this if the statistical error in phase is low, to try to prevent spuriously tracking
        // movements when fringe amplitude is too low to get useful information out of
        if interf.ref_laser.fit_coefficient_errs[2] < 1.0e-4 {
            let ref_adjustment = interf.ref_position_lock.do_pid(ref_error);
            interf
                .ref_position_lock
                .set_setpoint(interf.ref_position_lock.setpoint() + ref_adjustment);
        }
        // play the same game with the slave position lock to 'unroll' its movements over more than
        // a 2PI-radian interval
        if interf.slave_laser.fit_coefficient_errs[2] < 1.0e-4 {
            let slave_position_adjustment = interf.slave_position_lock.do_pid(slave_novel_error);
            interf
                .slave_position_lock
                .set_setpoint(interf.slave_position_lock.setpoint() + slave_position_adjustment);
        }

        let slave_adjustment = interf.slave_servo.do_pid(slave_absolute_error);
        let _ = slave_out_ch.adjust_offset(slave_adjustment);

        interf.ref_laser.phase_log.push(ref_error);
        interf.slave_laser.phase_log.push(slave_absolute_error);
        interf.slave_laser.feedback_log.push(slave_out_ch.offset());

        last_ref_result = ref_result;
        last_slave_result = slave_result;

        if interf_comms.should_publish_logs(interf.cycle_counter + 1) {
            // Ideally we'd always send the most recent waveform, but we handle communications
            // while the scope is acquiring, i.e. while the most recent waveform is being written in
            // memory. Instead, we have to copy the waveform ahead of time.
            let _ = match interf.ref_laser.input_channel {
                core::RPCoreChannel::CH_1 => pit.scope.write_raw_waveform(
                    &mut interf.last_waveform_ref,
                    &mut interf.last_waveform_slave,
                ),
                core::RPCoreChannel::CH_2 => pit.scope.write_raw_waveform(
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
