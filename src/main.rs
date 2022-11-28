#![allow(dead_code)]
#![allow(non_snake_case)]
#![warn(clippy::pedantic)]
#![warn(clippy::all)]

extern crate librp_sys;
extern crate zmq;

mod lib;
mod multifit;

// use data_structures::circle_buffer;
use lib::lock;

fn main() {
    let mut mylock = lock::Servo::new();
    mylock.gain_P = 1.0;
    mylock.gain_I = 0.1;
    mylock.enable();
    println!("{}", mylock.error_feedback());
    // println!("{:?}", mylock);
    mylock.do_pid(2.0);
    println!("{}", mylock.error_feedback());
    // println!("{:?}", mylock);
    mylock.do_pid(2.0);
    println!("{}", mylock.error_feedback());
    // println!("{:?}", mylock);
}
