// use std::path::Path;
use std::process::Command;

#[cfg(any(feature = "no_api", feature = "no_api_loud"))]
fn main() {
    println!("cargo:rerun-if-changed=librp-sys/include/rp.c");
    println!("cargo:rerun-if-changed=librp-sys/include/rp.h");
    println!("cargo:rerun-if-changed=build.rs");
}

#[cfg(not(any(feature = "no_api", feature = "no_api_loud")))]
fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let target = std::env::var("TARGET").unwrap();

    println!("cargo:rerun-if-changed=librp-sys/include/rp.c");
    println!("cargo:rerun-if-changed=librp-sys/include/rp.h");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-lib=rp");
    //TODO figure out where `make api` drops the `librp.so`
    println!(
        "cargo:rustc-link-search={}/RedPitaya/api/lib/librp.so",
        out_dir
    );

    Command::new("git")
        .arg("clone")
        .arg("https://github.com/RedPitaya/RedPitaya.git")
        .current_dir(&out_dir)
        .status()
        .unwrap();
    Command::new("git")
        .arg("checkout")
        .arg("v1.04-18")
        .current_dir(&format!("{}/RedPitaya", out_dir))
        .status()
        .unwrap();
    Command::new("make")
        .arg(&format!(
            "CROSS_COMPILE={}-",
            match &target[..] {
                "arm-unknown-linux-gnueabihf"
                | "arm-unknown-linux-gnueabi"
                | "armv7-unknown-linux-gnueabi" => "arm-linux-gnueabihf",
                "x86_64-pc-windows-gnu" => "x86_64-w64-mingw32",
                "x86_64-unknown-linux-gnu" => "x86_64-unknown-linux-gnu",
                _ => "",
            }
        ))
        .arg("api")
        .current_dir(&format!("{}/RedPitaya", &out_dir))
        .status()
        .unwrap();
}
