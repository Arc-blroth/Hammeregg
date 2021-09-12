use eframe::egui::Ui;

/// A "screen" in the Hammeregg UI.
pub trait Screen {
    /// Updates the UI for this screen. If this function
    /// returns some screen, that screen will be shown
    /// and replace this one next frame.
    fn update(&mut self, ui: &mut Ui) -> Option<Box<dyn Screen>>;
}
