use anyhow::Context;
use eframe::egui::Ui;
use hammeregg_core::RemotePassword;
use rsa::pkcs1::ToRsaPublicKey;
use rsa::RsaPrivateKey;

use crate::net;
use crate::net::WSS;
use crate::ui::screen::Screen;

pub struct RunningScreen {
    connected_label: String,
    password: Option<RemotePassword>,
}

impl RunningScreen {
    pub fn new(desktop_name: String, wss: WSS, password: (RsaPrivateKey, RsaPrivateKey)) -> Self {
        let (home_private_key, remote_private_key) = password;
        let home_public_key = home_private_key.to_public_key();
        let remote_public_key = remote_private_key.to_public_key();
        let handle = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(net::handle_signalling_requests(
                    wss,
                    home_private_key,
                    remote_public_key,
                ))
                .context("Couldn't handle signalling requests")
                .unwrap();
        });
        std::thread::spawn(move || {
            println!("{:?}", handle.join());
        });

        let home_public_pem = home_public_key.to_pkcs1_pem().unwrap();
        let remote_private_pem = remote_private_key.to_pkcs1_pem().unwrap();
        let password = Some(RemotePassword {
            home_public_key: home_public_pem,
            remote_private_key: remote_private_pem,
        });

        // Micro-optimization: pre-fill the entire desktop name label
        let connected_label = format!("Connected to signalling server as '{}'.", desktop_name);

        Self {
            connected_label,
            password,
        }
    }
}

impl Screen for RunningScreen {
    fn update(self: Box<Self>, ui: &mut Ui) -> (Box<dyn Screen>, bool) {
        ui.label(&self.connected_label);
        (self, false)
    }
}
