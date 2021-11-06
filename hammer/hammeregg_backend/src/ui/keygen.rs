use eframe::egui::Ui;
use futures::channel::oneshot;
use futures::channel::oneshot::Receiver;
use rsa::RsaPrivateKey;

use crate::key;
use crate::net::WSS;
use crate::ui::running::RunningScreen;
use crate::ui::screen::Screen;
use crate::ui::setup::SetupScreen;
use crate::work::WorkThread;

pub struct KeygenScreen {
    // these are passed through either back to
    // SetupScreen or forward to RunningScreen
    work_thread: WorkThread,
    desktop_name: String,
    signalling_server_addr: String,
    extra_ca: Option<String>,
    wss: WSS,

    error_msg: Option<String>,
    password_rx: Receiver<(RsaPrivateKey, RsaPrivateKey)>,
}

impl KeygenScreen {
    pub fn new(
        work_thread: WorkThread,
        desktop_name: String,
        signalling_server_addr: String,
        extra_ca: Option<String>,
        wss: WSS,
    ) -> Self {
        let (tx, rx) = oneshot::channel();
        std::thread::spawn(move || {
            // Key generation can take ~5 seconds even on a fast computer
            tx.send(key::gen_home_and_remote_keys()).unwrap();
        });
        Self {
            work_thread,
            desktop_name,
            signalling_server_addr,
            extra_ca,
            wss,
            error_msg: None,
            password_rx: rx,
        }
    }
}

impl Screen for KeygenScreen {
    fn update(mut self: Box<Self>, ui: &mut Ui) -> (Box<dyn Screen>, bool) {
        match &self.error_msg {
            Some(msg) => {
                ui.label("Couldn't generate keys :(");
                ui.colored_label(super::ERROR_COLOR, msg);
                ui.add_space(16.0);
                if ui.button("Back").clicked() {
                    (
                        Box::new(SetupScreen::recover(
                            self.work_thread,
                            self.desktop_name,
                            self.signalling_server_addr,
                            self.extra_ca,
                        )),
                        true,
                    )
                } else {
                    (self, false)
                }
            }
            None => {
                // Give some sort of loading indicator
                ui.label("Generating keys (this may take a few seconds)");

                // check to see if keygen finished
                match self.password_rx.try_recv() {
                    // still waiting
                    Ok(None) => (self, false),
                    // received error
                    Err(_) => {
                        self.error_msg = Some("Error: Key generation thread panicked".to_string());
                        (self, false)
                    }
                    Ok(Some(password)) => (
                        Box::new(RunningScreen::new(
                            self.work_thread,
                            self.desktop_name,
                            self.wss,
                            password,
                        )),
                        true,
                    ),
                }
            }
        }
    }
}
