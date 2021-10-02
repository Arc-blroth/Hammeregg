use std::ffi::CString;
use std::net::{IpAddr, SocketAddr};

use anyhow::Result;
use eframe::egui::{Button, Color32, Label, TextEdit, Ui};
use futures::channel::oneshot;
use futures::channel::oneshot::Receiver;
use hammeregg_core::DEFAULT_HAMMEREGG_PORT;

use crate::net;
use crate::net::WSS;
use crate::ui::running::RunningScreen;
use crate::ui::screen::Screen;

pub struct SetupScreen {
    desktop_name: String,
    signalling_server_addr: String,
    error_msg: Option<String>,
    signalling_connection_init: Option<Receiver<Result<WSS>>>,
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
            signalling_connection_init: None,
        }
    }
}

impl SetupScreen {
    /// Parses the `signalling_server_addr` into a valid
    /// [`SocketAddr`], adding the default Hammeregg
    /// signalling port if necessary. Returns `Some` on
    /// success and `None` on error.
    fn try_parse_signalling_server_addr(&self) -> Option<SocketAddr> {
        // First try to parse as a full ip:port
        match self.signalling_server_addr.parse::<SocketAddr>() {
            Ok(addr) => Some(addr),
            Err(_) => {
                // If that fails, try to parse as an ip and add on the port
                match self.signalling_server_addr.parse::<IpAddr>() {
                    Ok(ip) => Some(SocketAddr::new(ip, DEFAULT_HAMMEREGG_PORT)),
                    Err(_) => None,
                }
            }
        }
    }

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

        if self.try_parse_signalling_server_addr().is_none() {
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

    /// Starts the signalling connection init
    /// thread asynchronously.
    fn start_signalling_connection(&mut self) {
        let (tx, rx) = oneshot::channel();
        let desktop_name = self.desktop_name.clone();
        let addr = self.try_parse_signalling_server_addr().unwrap();
        std::thread::spawn(move || {
            let wss = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(net::init_signalling_connection(desktop_name, addr));
            tx.send(wss).unwrap();
        });
        self.signalling_connection_init = Some(rx);
    }

    /// Checks if the signalling connection is done
    /// initializing, returning the connection
    /// if initialization succeeded.
    fn check_signalling_connection(&mut self) -> Option<WSS> {
        match self.signalling_connection_init.as_mut().unwrap().try_recv() {
            // still waiting
            Ok(None) => None,
            // received error
            Err(_) => {
                self.error_msg = Some(format!("Error: Signalling thread panicked"));
                self.signalling_connection_init = None;
                None
            }
            Ok(Some(Err(err))) => {
                eprintln!("{:?}", err);
                self.error_msg = Some(format!("Error: {}", err));
                self.signalling_connection_init = None;
                None
            }
            // received success
            Ok(Some(Ok(wss))) => Some(wss),
        }
    }
}

impl Screen for SetupScreen {
    fn update(&mut self, ui: &mut Ui) -> Option<Box<dyn Screen>> {
        let enabled = self.signalling_connection_init.is_none();

        ui.heading("Hammeregg Config");
        ui.add_space(32.0);
        ui.horizontal(|ui| {
            ui.label("Desktop Name: ");
            ui.add(TextEdit::singleline(&mut self.desktop_name).enabled(enabled));
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Signalling Server: ");
            ui.add(TextEdit::singleline(&mut self.signalling_server_addr).enabled(enabled));
        });
        ui.add_space(4.0);
        ui.label(
            Label::new(self.error_msg.as_ref().unwrap_or(&String::default()))
                .text_color(Color32::from_rgb(245, 66, 66)),
        );
        ui.add_space(16.0);
        let start_clicked = ui.add(Button::new("Start!").enabled(enabled)).clicked();

        if enabled && start_clicked && self.validate_input() {
            self.start_signalling_connection();
            None
        } else if !enabled {
            match self.check_signalling_connection() {
                None => None,
                Some(wss) => Some(Box::new(RunningScreen::new())),
            }
        } else {
            None
        }
    }
}
