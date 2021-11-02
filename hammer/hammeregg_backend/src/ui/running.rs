use eframe::egui::Ui;

use crate::net::WSS;
use crate::ui::screen::Screen;

pub struct RunningScreen {
    wss: WSS,
}

impl RunningScreen {
    pub fn new(wss: WSS) -> Self {
        Self { wss }
    }
}

impl Screen for RunningScreen {
    fn update(&mut self, ui: &mut Ui) -> Option<Box<dyn Screen>> {
        ui.heading("Hammeregg Desktop");
        None
    }
}
