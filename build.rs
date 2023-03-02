use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("failed to get OUT_DIR");
    let target = std::env::var("TARGET").expect("failed to get TARGET");

    cargo_messages(&out_dir, &target);

    if !Path::new(&format!("{out_dir}/config.toml")).exists() {
        Command::new(format!("sh -c cp examples/config.toml {out_dir}/")).exec();
    }
    cc::Build::new()
        .file("src/multifit/sinusoid_fitting.c")
        .include("/usr/include/")
        .static_flag(true)
        .compile("sinusoid_fitting");
}

fn cargo_messages(out_dir: &str, target: &str) {
    println!("cargo:rerun-if-changed=src/multifit/sinusoid_fitting.c");
    println!("cargo:rerun-if-changed=src/multifit/sinusoid_fitting.h");
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rustc-link-lib=m");
    if target == "armv7-unknown-linux-gnueabihf" {
        println!("cargo:rustc-link-lib=static=gsl");
        println!("cargo:rustc-link-lib=static=gslcblas");
    } else {
        println!("cargo:rustc-link-lib=gsl");
        println!("cargo:rustc-link-lib=gslcblas");
    }
}
