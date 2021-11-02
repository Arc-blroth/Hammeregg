use eframe::egui::Ui;

/// A "screen" in the Hammeregg UI.
pub trait Screen {
    /// Updates the UI for this screen. The returned
    /// tuple's screen will be shown and updated next
    /// frame. If the tuple's second component is true,
    /// the UI will also be repacked next frame.
    fn update(self: Box<Self>, ui: &mut Ui) -> (Box<dyn Screen>, bool);
}
