//! Hammeregg's "backend" code, which handles
//! setting up the computer the backend runs
//! on for remote access.

#![feature(try_blocks)]
#![windows_subsystem = "windows"]

pub mod net;
pub mod pion;
pub mod ui;

fn main() {
    ui::show_ui();
}
