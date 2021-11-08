use std::rc::Rc;

use anyhow::{Context, Result};
use eframe::egui::Ui;
use rfd::FileDialog;
use rsa::pkcs8::{ToPrivateKey, ToPublicKey};
use rsa::RsaPrivateKey;

use crate::key::RemotePassword;
use crate::net;
use crate::net::WSS;
use crate::ui::screen::Screen;
use crate::work::WorkThread;

pub struct RunningScreen {
    work_thread: WorkThread,
    connected_label: String,
    password: Option<Rc<RemotePassword>>,
    error_msg: Option<String>,
}

impl RunningScreen {
    pub fn new(
        work_thread: WorkThread,
        desktop_name: String,
        wss: WSS,
        password: (RsaPrivateKey, RsaPrivateKey),
    ) -> Self {
        let (home_private_key, remote_private_key) = password;

        // start handling signalling requests
        let home_public_key = home_private_key.to_public_key();
        let remote_public_key = remote_private_key.to_public_key();
        let join_handle = work_thread.handle().spawn(net::handle_signalling_requests(
            wss,
            home_private_key,
            remote_public_key,
        ));
        work_thread.handle().spawn(async move {
            if let Err(err) = join_handle.await {
                eprintln!("Signalling loop panicked: {:?}", err);
            }
        });

        // generate the remote side of the Hammeregg password
        let home_public_pem = ToPublicKey::to_public_key_pem(&home_public_key).unwrap();
        let remote_private_pem = ToPrivateKey::to_pkcs8_pem(&remote_private_key).unwrap();
        // SAFETY: we make a copy of the private password
        // that is inserted into another Zeroizing struct.
        // Both the original private password and the new
        // password are zeroized once unneeded.
        let password = Some(Rc::new(RemotePassword {
            home_public_key: home_public_pem,
            remote_private_key: (&*remote_private_pem).clone(),
        }));

        // Micro-optimization: pre-fill the entire desktop name label
        let connected_label = format!("Connected to signalling server as '{}'.", desktop_name);

        Self {
            work_thread,
            connected_label,
            password,
            error_msg: None,
        }
    }
}

impl Screen for RunningScreen {
    fn update(mut self: Box<Self>, ui: &mut Ui) -> (Box<dyn Screen>, bool) {
        ui.label(&self.connected_label);

        // Save Password button
        if let Some(password) = self.password.clone() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("You'll need this password to connect to this computer: ");
                if ui.button("Save Password").clicked() {
                    if let Some(file) = FileDialog::new()
                        .set_title("Save Egg Password")
                        .add_filter("Egg Password", &["egps"])
                        .save_file()
                    {
                        let res: Result<()> = try {
                            let bson = bson::to_vec(&*password).context("Failed to serialize password")?;
                            std::fs::write(file, bson).context("Failed to write password")?;
                        };
                        match res {
                            Ok(_) => self.password = None,
                            Err(err) => self.error_msg = Some(format!("{}", err)),
                        }
                    }
                }
            });

            // error message in case something goes wrong in saving
            if let Some(msg) = &self.error_msg {
                ui.colored_label(super::ERROR_COLOR, msg);
            }
        } else {
            ui.label("Egg password saved!");
        }

        (self, false)
    }
}
