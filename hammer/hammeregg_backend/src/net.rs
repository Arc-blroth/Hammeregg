use std::ffi::{CStr, CString};
use std::net::SocketAddr;
use std::ops::Deref;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use atomic_refcell::{AtomicRefCell, AtomicRefMut};
use futures::channel::oneshot;
use futures::future::BoxFuture;
use futures::{future, FutureExt, SinkExt, StreamExt};
use hammeregg_core::{deserialize_packet, serialize_packet, HandshakeInitPacket, HandshakePacket, VERSION_1_0};
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rsa::{PaddingScheme, PublicKey, RsaPrivateKey, RsaPublicKey};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use url::Url;

use crate::pion;
use crate::pion::PeerConnection;

pub type WSS = WebSocketStream<MaybeTlsStream<TcpStream>>;

// Initializes a connection to the signalling server.
pub async fn init_signalling_connection(desktop_name: String, addr: SocketAddr) -> Result<WSS> {
    // Connect to the signalling server
    let mut url = Url::parse("ws://192.168.1.1:1234").unwrap();
    url.set_ip_host(addr.ip()).unwrap();
    url.set_port(Some(addr.port())).unwrap();
    let (mut socket, _) = tokio_tungstenite::connect_async(url)
        .await
        .context("Could not connect to signalling server")?;

    // Hammeregg Signalling Handshake
    // First, send a HomeInit packet to the signalling server.
    socket
        .send(serialize_packet(&HandshakeInitPacket::new(
            VERSION_1_0,
            HandshakePacket::HomeInit {
                home_name: desktop_name,
            },
        )?)?)
        .await?;

    // Then wait for a HomeInitResponse packet.
    match deserialize_packet::<HandshakePacket>(
        &socket
            .next()
            .await
            .context("Handshake failed: could not read packet")??,
    )? {
        HandshakePacket::HomeInitResponse { response } => response?,
        _ => Err(anyhow!(
            "Handshake failed: server did not respond HomeInitResponse to HomeInit"
        ))?,
    }

    Ok(socket)
}

async fn handle_signalling_requests(
    socket: WSS,
    home_private_key: RsaPrivateKey,
    remote_public_key: RsaPublicKey,
) -> Result<()> {
    let (send, recv) = socket.split();
    let connection_and_stop_future: AtomicRefCell<Option<(PeerConnection, Arc<AtomicBool>)>> = AtomicRefCell::new(None);
    recv.filter_map(|packet| {
        let res: Result<BoxFuture<Result<Message>>> = try {
            match deserialize_packet::<HandshakePacket>(&packet.context("Signalling failed: could not read packet")?)? {
                HandshakePacket::RemoteOffer { peer, payload } => {
                    // Do we already have a connection?
                    let can_accept = match connection_and_stop_future.borrow().deref() {
                        None => true,
                        Some((_, stop_notifier)) => stop_notifier.load(Ordering::SeqCst),
                    };
                    if !can_accept {
                        // If we're already connected to another client,
                        // return an error to the remote.
                        future::ready(serialize_packet(&HandshakePacket::HomeAnswerFailure {
                            peer,
                            error: "Another client is already connected".to_string(),
                        }))
                        .boxed()
                    } else {
                        handle_remote_offer(
                            connection_and_stop_future.borrow_mut(),
                            peer,
                            payload,
                            &home_private_key,
                            &remote_public_key,
                        )
                        .boxed()
                    }
                }
                _ => Err(anyhow!("Signalling failed: did not get a RemoteOffer packet"))?,
            }
        };
        match res {
            Ok(inner) => inner
                .then(|inner_res| {
                    future::ready(match inner_res {
                        Ok(inner) => Some(Ok(inner)),
                        Err(err) => {
                            eprintln!("{:?}", err);
                            None
                        }
                    })
                })
                .boxed(),
            Err(err) => {
                eprintln!("{:?}", err);
                future::ready(None).boxed()
            }
        }
    })
    .forward(send)
    .await?;
    Ok(())
}

async fn handle_remote_offer<'a>(
    mut connection_and_stop_future: AtomicRefMut<'a, Option<(PeerConnection, Arc<AtomicBool>)>>,
    peer: u32,
    payload: Vec<u8>,
    home_private_key: &RsaPrivateKey,
    remote_public_key: &RsaPublicKey,
) -> Result<Message> {
    let result = try {
        // Deserialize payload
        let decrypted_payload = home_private_key
            .decrypt(PaddingScheme::PKCS1v15Encrypt, payload.as_slice())
            .context("Signalling failed: couldn't decrypt remote payload")?;

        // Start the server
        let (connection, answer, stop_notifier) = start_pion_server(
            String::from_utf8(decrypted_payload).context("Signalling failed: offer was not a valid string")?,
        )
        .await?;

        // Encrypt answer payload
        let mut rng = ChaCha20Rng::from_entropy();
        let encrypted_answer = remote_public_key
            .encrypt(
                &mut rng,
                PaddingScheme::PKCS1v15Encrypt,
                answer.into_bytes().as_mut_slice(),
            )
            .context("Signalling failed: answer couldn't be encrypted")?;

        let message = serialize_packet(&HandshakePacket::HomeAnswerSuccess {
            peer,
            payload: encrypted_answer,
        })?;

        *connection_and_stop_future = Some((connection, stop_notifier));
        message
    };
    if let Err(_) = result {
        // Notify the remote that signalling failed
        Ok(serialize_packet(&HandshakePacket::HomeAnswerFailure {
            peer,
            error: "Signalling failed".to_string(),
        })?)
    } else {
        result
    }
}

/// Asynchronously starts a Pion RTP -> RTC server, blocking until
/// the server returns an answer to the given WebRTC offer.
/// Returns a pointer to the server's PeerConnection, the server's
/// answer, and an atomic boolean that will be set to true when
/// the server stops.
async fn start_pion_server(offer: String) -> Result<(PeerConnection, String, Arc<AtomicBool>)> {
    let (connection_tx, connection_rx) = oneshot::channel();
    let (answer_tx, answer_rx) = oneshot::channel();
    let stop_notifier = Arc::new(AtomicBool::new(false));
    let stop_notifier_out = stop_notifier.clone();
    std::thread::spawn(move || {
        /// A wrapper around a [`PeerConnection`] that ensures
        /// that the rtp2rtc server is cleaned up and that the
        /// wait-for-stop future is notified.
        struct PionServer {
            connection: PeerConnection,
            stop_notifier: Arc<AtomicBool>,
        }

        impl Drop for PionServer {
            fn drop(&mut self) {
                unsafe {
                    if !self.connection.is_null() {
                        pion::hammer_rtp2rtc_stop(self.connection);
                    }
                    self.stop_notifier.store(true, Ordering::SeqCst)
                }
            }
        }

        /// A wrapper around a `*mut c_char` returned from a call
        /// to Pion that ensures that `hammer_free_cstring` is
        /// cleaned up.
        #[repr(transparent)]
        struct PionCString {
            inner: *mut c_char,
        }

        impl Drop for PionCString {
            fn drop(&mut self) {
                if !self.inner.is_null() {
                    unsafe {
                        pion::hammer_free_cstring(self.inner);
                    }
                }
            }
        }

        // Init connection
        let connection = unsafe { pion::hammer_rtp2rtc_init() };
        if connection.is_null() {
            connection_tx
                .send(Err(anyhow!("RTP2RTC init failed: couldn't initialize connection!")))
                .unwrap();
            return;
        }
        connection_tx.send(Ok(connection)).unwrap();

        let server = PionServer {
            connection,
            stop_notifier,
        };

        // Signal offer
        const OFFER_ERR: &str = "RTP2RTC init failed: invalid offer!";
        let offer = match CString::new(offer) {
            Ok(string) => string,
            Err(_) => {
                answer_tx.send(Err(anyhow!(OFFER_ERR))).unwrap();
                return;
            }
        };
        let desc = unsafe {
            let offer = offer.as_bytes_with_nul();
            pion::hammer_rtp2rtc_build_offer(offer.as_ptr() as *const c_char, offer.len() as i32)
        };
        if desc.is_null() {
            answer_tx.send(Err(anyhow!(OFFER_ERR))).unwrap();
            return;
        }
        let answer_c = PionCString {
            inner: unsafe { pion::hammer_rtp2rtc_signal_offer(server.connection, desc) },
        };
        if answer_c.inner.is_null() {
            answer_tx.send(Err(anyhow!(OFFER_ERR))).unwrap();
            return;
        }
        let answer = match unsafe { CStr::from_ptr(answer_c.inner) }.to_str() {
            Ok(string) => string,
            Err(_) => {
                answer_tx.send(Err(anyhow!(OFFER_ERR))).unwrap();
                return;
            }
        };
        answer_tx.send(Ok(answer.to_string())).unwrap();

        // wait forever
        loop {
            std::thread::park();
        }
    });
    let connection = connection_rx.await??;
    let answer = answer_rx.await??;
    Ok((connection, answer, stop_notifier_out))
}
