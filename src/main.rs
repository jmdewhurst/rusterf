#![allow(dead_code)]
#![allow(non_snake_case)]
#![warn(clippy::pedantic)]
#![warn(clippy::all)]

extern crate toml;
mod lib;
mod multifit;
use lib::Laser;

fn main() {
    let mut las = Laser::new(4).unwrap();
    las.set_wavelength(1500.0, 3000.0, 1.5);

    println!("{:?}", las);

    let filename = "test.toml";
    let toml_string = toml::to_string(&las).expect("Could not encode TOML value");
    println!("{}", toml_string);
    std::fs::write(filename, toml_string).expect("Could not write to file!");

    let contents = std::fs::read_to_string(filename).unwrap();
    println!("{}", contents);
    let data: Laser = toml::from_str(&contents).unwrap();
    println!("final: {:?}", data);
}
