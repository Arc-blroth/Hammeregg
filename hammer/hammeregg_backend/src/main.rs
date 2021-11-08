//! Hammeregg's "backend" code, which handles
//! setting up the computer the backend runs
//! on for remote access.

#![feature(try_blocks)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod key;
pub mod net;
pub mod pion;
pub mod ui;
pub mod work;

fn main() {
    ui::show_ui();
}
