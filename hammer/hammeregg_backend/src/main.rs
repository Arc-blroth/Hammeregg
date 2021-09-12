//! Hammeregg's "backend" code, which handles
//! setting up the computer the backend runs
//! on for remote access.

#![windows_subsystem = "windows"]

pub mod pion;
pub mod ui;

fn main() {
    ui::show_ui();
}
