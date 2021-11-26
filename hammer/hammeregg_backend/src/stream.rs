use std::process::{Child, Command};

use anyhow::{Context, Result};

/// Streams the current desktop to the given rtp port.
pub fn stream_video(port: u16) -> Result<Child> {
    let address = format!("rtp://127.0.0.1:{}", port);
    Command::new("ffmpeg")
        .args([
            "-re",
            "-f",
            "gdigrab",
            "-i",
            "desktop",
            "-framerate",
            "30",
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
