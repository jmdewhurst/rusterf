use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let target = std::env::var("TARGET").unwrap();
    cargo_messages(&out_dir);

    // gsl(&out_dir, &target);

    // cc::Build::new()
    //     .file("src/multifit/sinusoid_fitting.c")
    //     .compile("sinusoid_fitting");
}

fn cargo_messages(out_dir: &str) {
    println!("cargo:rerun-if-changed=src/multifit/sinusoid_fitting.c");
    println!("cargo:rerun-if-changed=src/multifit/sinusoid_fitting.h");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-lib=gsl");
    println!("cargo:rustc-link-lib=gslcblas");
    println!("cargo:rustc-link-arg={}/gsl_compiled/lib/libgsl.a", out_dir);
    println!(
        "cargo:rustc-link-arg={}/gsl_compiled/lib/libgslcblas.a",
        out_dir
    );
}

fn gsl(out_dir: &str, target: &str) {
    // see https://coral.ise.lehigh.edu/jild13/2016/07/11/hello/
    let tar_exists = Path::new(&format!("{}/gsl-2.7.tar.gz", out_dir)).exists();
    if !tar_exists {
        Command::new("wget")
            .arg("ftp://ftp.gnu.org/gnu/gsl/gsl-2.7.tar.gz")
            .current_dir(out_dir)
            .status()
            .unwrap();
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
        .unwrap();
    Command::new("mkdir")
        .arg("gsl_compiled")
        .current_dir(out_dir)
        .status()
        .unwrap();

    match target {
        "arm-linux-gnueabihf"
        | "arm-unknown-linux-gnueabi"
        | "arm-unknown-linux-gnueabihf"
        | "armv7-unknown-linux-gnueabi" => Command::new("./configure")
            .arg("--host=arm-linux-gnueabihf")
            .arg(&format!("--prefix={}/gsl_compiled", out_dir))
            .current_dir(&format!("{}/gsl-2.7", out_dir))
            .status()
            .unwrap(),
        _ => Command::new("./configure")
            .arg(&format!("--prefix={}/gsl_compiled", out_dir))
            .current_dir(&format!("{}/gsl-2.7", out_dir))
            .status()
            .unwrap(),
    };
    Command::new("make")
        .current_dir(&format!("{}/gsl-2.7", out_dir))
        .status()
        .unwrap();
    Command::new("make")
        .arg("install")
        .current_dir(&format!("{}/gsl-2.7", out_dir))
        .status()
        .unwrap();
}
