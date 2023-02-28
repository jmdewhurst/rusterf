use std::path::Path;
use std::process::Command;

// Should be set to true if you want to build software that will run on the dev machine.
// Set to false to build software to run on the Red Pitaya
const HAS_NATIVE_GSL: bool = false;

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("failed to get OUT_DIR");
    let target = std::env::var("TARGET").expect("failed to get TARGET");
    // let prefix = match target.as_str() {
    //     "arm-linux-gnueabihf" | "arm-unknown-linux-gnueabihf" | "armv7-unknown-linux-gnueabihf" => {
    //         "arm-linux-gnueabihf-"
    //     }
    //     "arm-linux-musleabihf"
    //     | "arm-unknown-linux-musleabihf"
    //     | "armv7-unknown-linux-musleabihf" => "arm-linux-gnueabihf-",
    //     _ => "",
    // };
    // let target_gcc = format!("{}gcc", prefix);

    cargo_messages(&out_dir);

    // gsl(&out_dir, &target);

    if HAS_NATIVE_GSL {
        cc::Build::new()
            .file("src/multifit/sinusoid_fitting.c")
            .include("gsl_native/include/")
            .static_flag(true)
            .compile("sinusoid_fitting");
    } else {
        cc::Build::new()
            .file("src/multifit/sinusoid_fitting.c")
            .include("/usr/include/")
            .static_flag(true)
            .compile("sinusoid_fitting");
    }
}

fn cargo_messages(out_dir: &str) {
    println!("cargo:rerun-if-changed=src/multifit/sinusoid_fitting.c");
    println!("cargo:rerun-if-changed=src/multifit/sinusoid_fitting.h");
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rustc-link-lib=m");
    // println!("cargo:rustc-link-lib=gsl");
    // println!("cargo:rustc-link-lib=gslcblas");
    if cfg!(target = "armv7-unknown-linux-gnueabihf") {
        println!("cargo:rustc-link-lib=static=gsl");
        println!("cargo:rustc-link-lib=static=gslcblas");
    } else {
        println!("cargo:rustc-link-lib=gsl");
        println!("cargo:rustc-link-lib=gslcblas");
    }
}

fn gsl(out_dir: &str, target: &str) {
    // see https://coral.ise.lehigh.edu/jild13/2016/07/11/hello/
    let tar_exists = Path::new(&format!("{}/gsl-2.7.tar.gz", out_dir)).exists();
    if !tar_exists {
        Command::new("wget")
            .arg("ftp://ftp.gnu.org/gnu/gsl/gsl-2.7.tar.gz")
            .current_dir(out_dir)
            .status()
            .expect("failed wget");
    }

    // Don't care about success, because this dir might not exist in the first place
    let _ = Command::new("rm")
        .arg("-r")
        .arg("gsl-2.7")
        .current_dir(out_dir)
        .status();

    Command::new("tar")
        .arg("-zxvf")
        .arg("gsl-2.7.tar.gz")
        .current_dir(out_dir)
        .status()
        .expect("failed tar");
    Command::new("mkdir")
        .arg("gsl_compiled")
        .current_dir(out_dir)
        .status()
        .expect("failed to mkdir");

    match target {
        "arm-linux-gnueabihf"
        | "arm-unknown-linux-gnueabi"
        | "arm-unknown-linux-gnueabihf"
        | "armv7-unknown-linux-gnueabi" => Command::new("./configure")
            .arg("--host=arm-linux-gnueabihf")
            .arg(&format!("--prefix={}/gsl_compiled", out_dir))
            .current_dir(&format!("{}/gsl-2.7", out_dir))
            .status()
            .expect("failed configure"),
        _ => Command::new("./configure")
            .arg(&format!("--prefix={}/gsl_compiled", out_dir))
            .current_dir(&format!("{}/gsl-2.7", out_dir))
            .status()
            .expect("failed configure"),
    };
    Command::new("make")
        .current_dir(&format!("{}/gsl-2.7", out_dir))
        .status()
        .expect("failed to make");
    Command::new("make")
        .arg("install")
        .current_dir(&format!("{}/gsl-2.7", out_dir))
        .status()
        .expect("failed make install");
}
