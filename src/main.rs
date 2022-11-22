#[allow(dead_code)]
#[warn(clippy::pedantic)]
#[warn(clippy::all)]
#[allow(non_snake_case)]
mod data_structures;

// use data_structures::circle_buffer;
use data_structures::lock;

fn main() {
    let mut mylock = lock::Servo::new();
    mylock.gain_P = 1.0;
    mylock.gain_I = 0.1;
    mylock.enable();
    println!("{}", mylock.error_feedback());
    println!("{:?}", mylock);
    mylock.new_error(2.0);
    println!("{}", mylock.error_feedback());
    println!("{:?}", mylock);
    mylock.new_error(2.0);
    println!("{}", mylock.error_feedback());
    println!("{:?}", mylock);
}
