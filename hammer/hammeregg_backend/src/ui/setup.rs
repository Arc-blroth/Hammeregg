use std::ffi::CString;
use std::net::{IpAddr, SocketAddr};

use anyhow::Result;
use eframe::egui::{Button, Label, TextEdit, Ui};
use futures::channel::oneshot::Receiver;
use hammeregg_core::DEFAULT_HAMMEREGG_PORT;

use crate::net;
use crate::net::WSS;
use crate::ui::keygen::KeygenScreen;
use crate::ui::screen::Screen;
use crate::work::WorkThread;

pub struct SetupScreen {
    work_thread: WorkThread,
    desktop_name: String,
    signalling_server_addr: String,
    extra_ca: Option<String>,
    error_msg: Option<String>,
    signalling_connection_init: Option<Receiver<Result<WSS>>>,
}

impl SetupScreen {
    /// Creates a new SetupScreen with the
    /// `desktop_name` field set to a random
    /// value and all other fields blank.
    pub fn new(work_thread: WorkThread) -> Self {
        Self {
            work_thread,
            desktop_name: names::Generator::default().next().unwrap(),
            signalling_server_addr: String::default(),
            extra_ca: None,
            error_msg: None,
            signalling_connection_init: None,
        }
    }

    /// Creates a new SetupScreen with prefilled
    /// fields.
    pub fn recover(
        work_thread: WorkThread,
        desktop_name: String,
        signalling_server_addr: String,
        extra_ca: Option<String>,
    ) -> Self {
        Self {
            work_thread,
            desktop_name,
            signalling_server_addr,
            extra_ca,
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
        let desktop_name = self.desktop_name.clone();
        let addr = self.try_parse_signalling_server_addr().unwrap();
        let extra_ca = self.extra_ca.clone();
        let rx = self
            .work_thread
            .spawn_task(net::init_signalling_connection(desktop_name, addr, extra_ca));
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
                self.error_msg = Some("Error: Signalling thread panicked".to_string());
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
    fn update(mut self: Box<Self>, ui: &mut Ui) -> (Box<dyn Screen>, bool) {
        let enabled = self.signalling_connection_init.is_none();

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
        ui.horizontal(|ui| {
            ui.label("Root CA (Optional): ").on_hover_text("An additional root certificate authority to trust when\nauthenticating the signalling server connection.");
            let mut editable_ca_field = self.extra_ca.clone().unwrap_or("".to_string());
            ui.add(TextEdit::singleline(&mut editable_ca_field).enabled(enabled));
            self.extra_ca = if editable_ca_field.trim().is_empty() { None } else { Some(editable_ca_field) };
        });
        ui.add_space(4.0);
        ui.label(Label::new(self.error_msg.as_ref().unwrap_or(&String::default())).text_color(super::ERROR_COLOR));
        ui.add_space(16.0);
        let start_clicked = ui.add(Button::new("Start!").enabled(enabled)).clicked();

        if enabled && start_clicked && self.validate_input() {
            self.start_signalling_connection();
            (self, false)
        } else if !enabled {
            match self.check_signalling_connection() {
                None => (self, false),
                Some(wss) => (
                    Box::new(KeygenScreen::new(
                        self.work_thread,
                        self.desktop_name,
                        self.signalling_server_addr,
                        self.extra_ca,
                        wss,
                    )),
                    true,
                ),
            }
        } else {
            (self, false)
        }
    }
}
