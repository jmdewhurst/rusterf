use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("failed to get OUT_DIR");
    let target = std::env::var("TARGET").expect("failed to get TARGET");

    cargo_messages(&out_dir);

    if !Path::new(&format!("{out_dir}/config.toml")).exists() {
        Command::new(format!("sh -c cp examples/config.toml {out_dir}/")).exec();
    }
    cc::Build::new()
        .file("src/multifit/sinusoid_fitting.c")
        .include("/usr/include/")
        .static_flag(true)
        .compile("sinusoid_fitting");
}

fn cargo_messages(out_dir: &str) {
    println!("cargo:rerun-if-changed=src/multifit/sinusoid_fitting.c");
    println!("cargo:rerun-if-changed=src/multifit/sinusoid_fitting.h");
    println!("cargo:rerun-if-changed=build.rs");

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").expect("failed to get target arch");

    // if target_arch == "arm" {
    println!("cargo:rustc-link-lib=static=gsl");
    println!("cargo:rustc-link-lib=static=gslcblas");
    println!("cargo:rustc-link-lib=static=zmq");
    // zmq dependencies?
    // println!("cargo:rustc-link-lib=static=sodium");
    // println!("cargo:rustc-link-lib=static=curl");
    // println!("cargo:rustc-link-lib=static=gssapi");
    // println!("cargo:rustc-link-lib=static=pgm");
    // println!("cargo:rustc-link-lib=static=norm");
    // println!("cargo:rustc-link-lib=static=protobuf");
    // println!("cargo:rustc-link-lib=static=protolib");
    // } else {
    //     println!("cargo:rustc-link-lib=gsl");
    //     println!("cargo:rustc-link-lib=gslcblas");
    //     println!("cargo:rustc-link-lib=zmq");
    // }
}
