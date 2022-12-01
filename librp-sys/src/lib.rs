#[warn(clippy::pedantic)]
#[warn(clippy::all)]
extern crate serde;

pub mod core;
pub mod dpin;
pub mod generator;
pub mod oscilloscope;
pub mod pitaya;

mod resources;
