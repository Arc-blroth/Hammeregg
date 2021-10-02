use eframe::egui::Ui;

use crate::ui::screen::Screen;

pub struct RunningScreen {}

impl RunningScreen {
    pub fn new() -> Self {
        Self {}
    }
}

impl Screen for RunningScreen {
    fn update(&mut self, ui: &mut Ui) -> Option<Box<dyn Screen>> {
        ui.heading("Hammeregg Desktop");
        None
    }
}
