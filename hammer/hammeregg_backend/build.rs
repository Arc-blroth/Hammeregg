#![feature(exit_status_error)]

use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Rerun this script if `rtp2rtc` is edited.
    println!("cargo:rerun-if-changed=../rtp2rtc");

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let a_path = out_path.join("librtp2rtc.a");
    Command::new("go")
        .current_dir("../rtp2rtc")
        .env("CGO_ENABLED", "1")
        .args([
            "build",
            "-buildmode=c-archive",
            "-o",
            a_path.display().to_string().as_str(),
        ])
        .spawn()
        .expect("Could not start go build!")
        .wait()
        .expect("Go process isn't running?")
        .exit_ok()
        .expect("Building rtp2rtc failed!");

    println!("cargo:rustc-link-search=native={}", out_path.display());
    println!("cargo:rustc-link-lib=static={}", "rtp2rtc");
}
