use crate::ui::screen::Screen;
use eframe::egui::{Ui, Label, Color32};
use std::ffi::CString;
use std::net::SocketAddr;
use crate::ui::running::RunningScreen;

pub struct SetupScreen {
    desktop_name: String,
    signalling_server_addr: String,
    error_msg: Option<String>,
}

impl SetupScreen {
    /// Creates a new SetupScreen with the
    /// `desktop_name` field set to a random
    /// value and all other fields blank.
    pub fn new() -> Self {
        Self {
            desktop_name: names::Generator::default().next().unwrap(),
            signalling_server_addr: String::default(),
            error_msg: None,
        }
    }
}

impl SetupScreen {
    /// Validates that:
    /// - `desktop_name` is not empty and a valid CString
    /// - `error_msg` is a valid [`IpAddr`]
    /// If validation fails, this will set the `error_msg`
    /// and return false.
    ///
    /// [`IpAddr`]: std::net::ip::IpAddr
    fn validate_input(&mut self) -> bool {
        let mut valid = true;
        let mut errors = vec![];

        if self.desktop_name.is_empty() {
            valid = false;
            errors.push("desktop name cannot be empty");
        } else if CString::new(self.desktop_name.clone()).is_err() {
            valid = false;
            errors.push("desktop name cannot contain '\\0'");
        }

        if self.signalling_server_addr.parse::<SocketAddr>().is_err() {
            valid = false;
            errors.push("signalling server is not a valid ip:port");
        }

        if errors.is_empty() {
            self.error_msg = None;
        } else {
            self.error_msg = Some(format!(
                "Error{}: {}!",
                if errors.len() == 1 { "" } else { "s" },
                errors.join(", ")
            ));
        }

        valid
    }
}

impl Screen for SetupScreen {
    fn update(&mut self, ui: &mut Ui) -> Option<Box<dyn Screen>> {
        ui.heading("Hammeregg Config");
        ui.add_space(32.0);
        ui.horizontal(|ui| {
            ui.label("Desktop Name: ");
            ui.text_edit_singleline(&mut self.desktop_name);
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Signalling Server: ");
            ui.text_edit_singleline(&mut self.signalling_server_addr);
        });
        ui.add_space(4.0);
        ui.label(
            Label::new(self.error_msg.as_ref().unwrap_or(&String::default()))
                .text_color(Color32::from_rgb(245, 66, 66))
        );
        ui.add_space(16.0);
        if ui.button("Start!").clicked() && self.validate_input() {
            Some(Box::new(RunningScreen::new(
                self.desktop_name.clone(),
                self.signalling_server_addr.parse().unwrap(),
            )))
        } else {
            None
        }
    }
}
