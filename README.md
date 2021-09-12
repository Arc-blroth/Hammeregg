<div align="center">
    <h1>Hammeregg</h1>
    <h3><i>all shells are ephemereal</i></h3>
    <br>
</div>

Hammeregg is a remote desktop that lets you access your home computer from any browser. It leverages the WebRTC API to provide a secure, real time desktop perfect for grabbing files you forgot at home or for testing the true strength of eggshells.

## Building

Hammeregg requires [Rust](https://www.rust-lang.org/), [Go](https://golang.org/), and [Node](https://nodejs.org/) to build, as well as a copy of [FFmpeg](https://www.ffmpeg.org/) to run. The backend server is implemented in `hammer/` and frontend content is implemented in `egg/`.

To build the backend, run
```sh
cd hammer
cargo build
```
Note that on Windows, the `pc-windows-gnu` toolchain is required.