use std::net::SocketAddr;

use eframe::egui::Ui;

use crate::pion::{hammer_rtp2rtc_init, PeerConnection};
use crate::ui::screen::Screen;

pub struct RunningScreen {
    connection: PeerConnection,
}

impl RunningScreen {
    pub fn new(desktop_name: String, signalling_server: SocketAddr) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let connection = unsafe { hammer_rtp2rtc_init() };
            tx.send(connection).unwrap();
        });
        let connection = rx.recv().unwrap();
        Self { connection }
    }
}

impl Screen for RunningScreen {
    fn update(&mut self, ui: &mut Ui) -> Option<Box<dyn Screen>> {
        ui.heading("Hammeregg Desktop");
        None
    }
}
