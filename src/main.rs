#![warn(clippy::pedantic)]
#![warn(clippy::all)]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(clippy::cast_precision_loss)]
//TODO: remove these allows
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::env;
use std::f32::consts::PI;
use std::fs::read_to_string;

use librp_sys::oscilloscope::Oscilloscope;
use rand::distributions::{Distribution, Uniform};
// extern crate toml;
extern crate zmq;

use librp_sys::Pitaya;
use librp_sys::*;

mod lib;
mod multifit;

use crate::lib::configs::{comms_from_config, interferometer_from_config};
use crate::lib::interferometer::Interferometer;
// use lib::laser::Laser;
use crate::multifit::{sinusoid, wrapped_angle_difference, FitSetup};

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
    let interf = interferometer_from_config(&cfg_text)
        .expect("Failed to construct interferometer object from config file");
    let interf_comms =
        comms_from_config(&cfg_text, ctx).expect("Failed to construct sockets from config file");

    if interf.is_master {
        println!("Designated as MASTER RP; controlling interferometer voltage ramp");
    }

    let scope = Oscilloscope::init(&mut pit);
    todo!("initialize ramp and oscilloscope!");
}
