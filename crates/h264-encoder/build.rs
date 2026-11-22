//! Builds the C shim around libx264 and links it, when libx264's header is there. Without it
//! (a machine that never runs the lab), the crate builds a stub whose encoder never opens, so
//! `cargo test --workspace` works anywhere.

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Where Homebrew puts libraries on Apple silicon.
const HOMEBREW_PREFIX: &str = "/opt/homebrew";

fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_x264)");
    println!("cargo::rerun-if-changed=shim/x264_shim.c");
    println!("cargo::rerun-if-env-changed=CC");
    let Some(out) = env::var_os("OUT_DIR").map(PathBuf::from) else {
        return;
    };
    let c_compiler = env::var("CC").unwrap_or_else(|_| "cc".to_owned());
    let mut extra: Vec<String> = Vec::new();
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        extra.push(format!("-I{HOMEBREW_PREFIX}/include"));
        println!("cargo::rustc-link-search=native={HOMEBREW_PREFIX}/lib");
    }
    let object = out.join("x264_shim.o");
    let shim_compiled = Command::new(&c_compiler)
        .args(["-O2", "-fPIC", "-c", "shim/x264_shim.c", "-o"])
        .arg(&object)
        .args(&extra)
        .status()
        .is_ok_and(|status| status.success());
    let archived = shim_compiled
        && Command::new("ar")
            .arg("rcs")
            .arg(out.join("libfreewheel_x264_shim.a"))
            .arg(&object)
            .status()
            .is_ok_and(|status| status.success());
    if !archived {
        println!("cargo::warning=libx264 not found: lab-encoder is a stub (install libx264-dev)");
        return;
    }
    println!("cargo::rustc-link-search=native={}", out.display());
    println!("cargo::rustc-link-lib=static=freewheel_x264_shim");
    println!("cargo::rustc-link-lib=x264");
    println!("cargo::rustc-cfg=has_x264");
}
