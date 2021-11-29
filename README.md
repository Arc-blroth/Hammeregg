<div align="center">
    <h1>Hammeregg</h1>
    <h3><i>all shells are ephemereal</i></h3>
    <br>
</div>

Hammeregg is a remote desktop that lets you access your home computer from any browser. It leverages the WebRTC API to provide a secure, real time desktop perfect for grabbing files you forgot at home or for testing the true strength of eggshells.

## Overview

Hammeregg consists of three components, all of which work together:

### 🔨 [Desktop](hammer/hammeregg_backend)
Hammeregg's "backend", which runs the remote desktop server on your home computer.

### 🐓 [Rooster](hammer/hammeregg_rooster)
Hammeregg's signalling server, which is used to perform the initial offer/answer between your browser and your home computer.

### 🥚 [Egg](egg)
Hammeregg's "frontend", a single-page app that lets you remotely connect to your home computer.

## Building

Hammeregg requires [Rust](https://www.rust-lang.org/), [Go](https://golang.org/), [Node](https://nodejs.org/), and [Yarn](https://yarnpkg.com/) to build, as well as a copy of [FFmpeg](https://www.ffmpeg.org/) to run.

To build both Desktop and Rooster, run
```sh
cd hammer
cargo build --release
```
Note that on Windows, the `pc-windows-gnu` toolchain for Rust is required.

To build Egg, run
```sh
cd egg
yarn install
yarn parcel build src/index.html
```
