pub mod running;
pub mod screen;
pub mod setup;

use eframe::egui::{Align2, CtxRef, FontDefinitions, Rgba, Vec2, Window};
use eframe::epi::{App, Frame, Storage};
use eframe::NativeOptions;

use crate::ui::screen::Screen;
use crate::ui::setup::SetupScreen;

const APP_NAME: &str = "Hammeregg Desktop";
const WINDOW_PADDING: Vec2 = Vec2::splat(16.0);

/// The main Hammeregg UI app.
pub struct UI {
    current_screen: Box<dyn Screen>,
    packed: bool,
    clear_color: Option<Rgba>,
}

impl UI {
    /// Creates a UI with the given screen.
    pub fn new<S: Screen + 'static>(screen: S) -> Self {
        Self {
            current_screen: Box::new(screen),
            packed: false,
            clear_color: None,
        }
    }
}

impl App for UI {
    fn update(&mut self, ctx: &CtxRef, frame: &mut Frame<'_>) {
        Window::new(APP_NAME)
            .title_bar(false)
            .resizable(false)
            .anchor(Align2::LEFT_TOP, Vec2::default())
            .frame(eframe::egui::Frame {
                margin: WINDOW_PADDING,
                corner_radius: 0.0,
                fill: ctx.style().visuals.window_fill(),
                stroke: Default::default(),
                ..Default::default()
            })
            .show(ctx, |ui| {
                ui.style_mut().spacing.button_padding = Vec2::new(16.0, 4.0);

                let maybe_new_screen = self.current_screen.update(ui);
                // Pack the window if the screen changed last frame.
                if !self.packed {
                    frame.set_window_size(ui.min_size() + 2.0 * WINDOW_PADDING);
                    self.packed = true;
                }
                if let Some(new_screen) = maybe_new_screen {
                    self.current_screen = new_screen;
                    self.packed = false;
                }
            });
    }

    fn setup(&mut self, ctx: &CtxRef, _frame: &mut Frame<'_>, _storage: Option<&dyn Storage>) {
        // Make font sizes not microscopic
        let mut definitions = FontDefinitions::default();
        definitions.family_and_size.values_mut().for_each(|x| x.1 *= 1.2);
        ctx.set_fonts(definitions);

        self.clear_color = Some(ctx.style().visuals.window_fill().into());
    }

    fn name(&self) -> &str {
        APP_NAME
    }

    fn clear_color(&self) -> Rgba {
        self.clear_color.unwrap()
    }
}

/// Shows the Hammeregg UI.
pub fn show_ui() {
    let options = NativeOptions {
        initial_window_size: Some(Vec2::default()),
        ..NativeOptions::default()
    };
    eframe::run_native(Box::new(UI::new(SetupScreen::new())), options);
}
