use std::process::{Child, Command};

use anyhow::{Context, Result};

/// The logical pixel boundaries of
/// the monitor that Hammeregg is sharing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MonitorBounds {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl MonitorBounds {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }
}

/// Streams the given monitor to the given rtp port.
pub fn stream_video(monitor_bounds: MonitorBounds, port: u16) -> Result<Child> {
    let offset_x = monitor_bounds.x.to_string();
    let offset_y = monitor_bounds.y.to_string();
    let video_size = format!("{}x{}", monitor_bounds.w, monitor_bounds.h);
    let address = format!("rtp://127.0.0.1:{}", port);
    Command::new("ffmpeg")
        .args([
            "-re",
            "-f",
            "gdigrab",
            "-framerate",
            "30",
            "-offset_x",
            offset_x.as_str(),
            "-offset_y",
            offset_y.as_str(),
            "-video_size",
            video_size.as_str(),
            "-show_region",
            "1",
            "-i",
            "desktop",
            "-vf",
            "scale='min(1280,iw)':-2",
            "-vcodec",
            "libvpx",
            "-cpu-used",
            "5",
            "-deadline",
            "1",
            "-crf",
            "30",
            "-b:v",
            "2M",
            "-g",
            "10",
            "-auto-alt-ref",
            "0",
            "-f",
            "rtp",
            address.as_str(),
        ])
        .spawn()
        .context("Couldn't start ffmpeg!")
}
