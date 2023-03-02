#[cfg(any(feature = "no_api", feature = "no_api_loud"))]
fn main() {
    println!("cargo:rerun-if-changed=librp-sys/include/rp.c");
    println!("cargo:rerun-if-changed=librp-sys/include/rp.h");
    println!("cargo:rerun-if-changed=build.rs");
}

#[cfg(not(any(feature = "no_api", feature = "no_api_loud")))]
fn main() {
    println!("cargo:rerun-if-changed=librp-sys/include/rp.c");
    println!("cargo:rerun-if-changed=librp-sys/include/rp.h");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-lib=static=rp");
    println!("cargo:rustc-link-search=librp-sys/obj/")
}
