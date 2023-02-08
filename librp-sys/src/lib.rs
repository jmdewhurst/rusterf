#![warn(clippy::pedantic)]
#![warn(clippy::all)]
extern crate serde;

#[macro_use]
pub mod core;
pub mod dpin;
pub mod generator;
pub mod oscilloscope;
pub mod pitaya;

pub use pitaya::Pitaya;
