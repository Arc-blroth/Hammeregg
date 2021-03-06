use std::ffi::{c_void, CStr, CString};
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::os::raw::c_char;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key as AesGcmKey, NewAead, Nonce};
use anyhow::{anyhow, Context, Result};
use atomic_refcell::{AtomicRefCell, AtomicRefMut};
use enigo::Enigo;
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::{future, FutureExt, SinkExt, StreamExt};
use hammeregg_core::{
    deserialize_packet, serialize_packet, HandshakeInitPacket, HandshakePacket, InputPacket, VERSION_1_0,
};
use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rsa::{PublicKey, RsaPrivateKey, RsaPublicKey};
use tokio::net::TcpStream;
use tokio_rustls::rustls::{Certificate, ClientConfig, OwnedTrustAnchor, RootCertStore};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{client_async_tls_with_config, Connector, MaybeTlsStream, WebSocketStream};
use url::Url;
use zeroize::Zeroizing;

use crate::pion::{make_c_closure, PeerConnection};
use crate::stream::MonitorBounds;
use crate::{input, key, pion, stream};

pub type WSS = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Stores the components underlying a single remote connection.
struct RemoteConnection {
    connection: PeerConnection,
    stop_notifier: Arc<AtomicBool>,
    video_process: Child,
}

impl Drop for RemoteConnection {
    fn drop(&mut self) {
        if !self.connection.is_null() && !self.stop_notifier.load(Ordering::SeqCst) {
            unsafe {
                pion::hammer_rtp2rtc_stop(self.connection);
            }
            if let Err(err) = self.video_process.kill() {
                eprintln!("Couldn't kill video process: {}", err);
            }
        }
    }
}

// Initializes a connection to the signalling server.
pub async fn init_signalling_connection(
    desktop_name: String,
    addr: SocketAddr,
    extra_ca: Option<String>,
) -> Result<WSS> {
    println!("Connecting to signalling server {} with name {}", addr, desktop_name);

    // Setup TLS
    let mut root_store = RootCertStore::empty();
    root_store.add_server_trust_anchors(
        webpki_roots::TLS_SERVER_ROOTS
            .0
            .iter()
            .map(|x| OwnedTrustAnchor::from_subject_spki_name_constraints(x.subject, x.spki, x.name_constraints)),
    );

    // Load ca.crt, if it exists
    if let Some(extra_ca_path) = extra_ca {
        let file = File::open(extra_ca_path).context("Couldn't open root certificate")?;
        let certs: Vec<_> = rustls_pemfile::certs(&mut BufReader::new(file))
            .map(|mut certs| certs.drain(..).map(Certificate).collect())?;
        for cert in certs {
            root_store.add(&cert)?;
        }
    }

    let mut config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    // TODO THIS IS POSSIBLY A BAD IDEA
    // Since webpki doesn't handle IP SANs (see briansmith/webpki#54),
    // SNI is guaranteed to fail from Hammeregg Desktop.
    // This line of code should be removed once IP SANs are finally
    // implemented in webpki.
    config.enable_sni = false;

    let connector = Connector::Rustls(Arc::new(config));

    // Connect to the signalling server
    // The server **must** present a certificate with hammeregg.default as a SAN.
    let url = Url::parse("wss://hammeregg.default").unwrap();

    let stream = TcpStream::connect(addr)
        .await
        .context("Couldn't connect to signalling server")?;
    let (mut socket, _) = client_async_tls_with_config(url, stream, None, Some(connector))
        .await
        .context("Couldn't connect to signalling server: TLS or WebSocket handshake failed")?;

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

pub async fn handle_signalling_requests(
    socket: WSS,
    home_private_key: RsaPrivateKey,
    remote_public_key: RsaPublicKey,
    monitor_bounds: MonitorBounds,
) -> Result<()> {
    let (mut send, mut recv) = socket.split();
    let remote_ref: AtomicRefCell<Option<RemoteConnection>> = AtomicRefCell::new(None);

    println!("Handling signalling requests!");
    // purely functional version of this loop blocked on https://github.com/rust-lang/rust/issues/90656
    while let Some(packet) = recv.next().await {
        let res: Result<BoxFuture<Result<Message>>> = try {
            match deserialize_packet::<HandshakePacket>(&packet.context("Signalling failed: could not read packet")?)? {
                HandshakePacket::RemoteOffer { peer, key, iv, payload } => handle_remote_offer(
                    remote_ref.borrow_mut(),
                    peer,
                    key,
                    iv,
                    payload,
                    &home_private_key,
                    &remote_public_key,
                    monitor_bounds,
                )
                .boxed(),
                _ => Err(anyhow!("Signalling failed: did not get a RemoteOffer packet"))?,
            }
        };
        match res {
            Ok(inner) => inner
                .then(|inner_res| match inner_res {
                    Ok(inner) => send.send(inner).boxed(),
                    Err(err) => {
                        eprintln!("{:?}", err);
                        future::ready(Ok(())).boxed()
                    }
                })
                .await
                .map_err(|x| {
                    let out = anyhow!(x);
                    eprintln!("Signalling handler loop crashed: {:?}", out);
                    out
                })?,
            Err(err) => {
                eprintln!("{:?}", err);
            }
        }
    }
    Ok(())
}

async fn handle_remote_offer<'a>(
    mut remote_ref: AtomicRefMut<'a, Option<RemoteConnection>>,
    peer: u32,
    key: Vec<u8>,
    iv: Vec<u8>,
    payload: Vec<u8>,
    home_private_key: &RsaPrivateKey,
    remote_public_key: &RsaPublicKey,
    monitor_bounds: MonitorBounds,
) -> Result<Message> {
    println!(
        "Handling remote offer from peer {} with payload length {}",
        peer,
        payload.len()
    );

    let result = try {
        // Quick sanity check: does the init vector make sense?
        if iv.len() != key::AES_IV_SIZE {
            Err(anyhow!("Invalid AES init vector length {}", iv.len()))?;
        }

        // Deserialize key
        let decrypted_key = home_private_key
            .decrypt(key::padding_scheme(), key.as_slice())
            .context("Signalling failed: couldn't decrypt remote key")?;

        // Quick sanity check: does the key length make sense?
        if decrypted_key.len() != key::AES_KEY_SIZE {
            Err(anyhow!("Invalid AES key length {}", decrypted_key.len()))?;
        }

        // Deserialize payload
        let aes_cipher = Aes256Gcm::new(AesGcmKey::from_slice(decrypted_key.as_slice()));
        let decrypted_payload = aes_cipher
            .decrypt(iv.as_slice().into(), payload.as_slice())
            .map_err(|_| anyhow!("Signalling failed: couldn't decrypt remote payload"))?;

        // Start the server
        let (new_remote, answer) = start_pion_server(
            String::from_utf8(decrypted_payload).context("Signalling failed: offer was not a valid string")?,
            monitor_bounds,
        )
        .await?;

        // Encrypt answer payload
        let mut rng = ChaCha20Rng::from_entropy();

        let mut out_key_data = Zeroizing::new(Vec::with_capacity(key::AES_KEY_SIZE));
        out_key_data.resize(key::AES_KEY_SIZE, 0);
        rng.try_fill_bytes(out_key_data.as_mut_slice())
            .context("Signalling failed: couldn't generate AES key")?;
        let out_key = AesGcmKey::from_slice(out_key_data.as_slice());

        let mut out_iv_data = Vec::with_capacity(key::AES_IV_SIZE);
        out_iv_data.resize(key::AES_IV_SIZE, 0);
        rng.try_fill_bytes(out_iv_data.as_mut_slice())
            .context("Signalling failed: couldn't generate AES init vector")?;
        let out_nonce = Nonce::from_slice(out_iv_data.as_slice());

        let out_aes_cipher = Aes256Gcm::new(out_key);

        let encrypted_key = remote_public_key
            .encrypt(&mut rng, key::padding_scheme(), &**out_key_data)
            .context("Signalling failed: key couldn't be encrypted")?;
        let encrypted_answer = out_aes_cipher
            .encrypt(out_nonce, answer.into_bytes().as_slice())
            .map_err(|_| anyhow!("Signalling failed: answer couldn't be encrypted"))?;

        let message = serialize_packet(&HandshakePacket::HomeAnswerSuccess {
            peer,
            key: encrypted_key,
            iv: out_iv_data,
            payload: encrypted_answer,
        })?;

        remote_ref.replace(new_remote);
        message
    };
    if let Err(err) = result {
        eprintln!("{:?}", err);
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
async fn start_pion_server(offer: String, monitor_bounds: MonitorBounds) -> Result<(RemoteConnection, String)> {
    let (connection_tx, connection_rx) = oneshot::channel();
    let (answer_tx, answer_rx) = oneshot::channel();
    let (ports_tx, mut ports_rx) = mpsc::unbounded();
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
                self.stop_notifier.store(true, Ordering::SeqCst);
                unsafe {
                    if !self.connection.is_null() {
                        pion::hammer_rtp2rtc_stop(self.connection);
                        pion::hammer_rtp2rtc_free(self.connection);
                    }
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

        // Start streaming!
        start_pion_server_inner(connection, ports_tx, monitor_bounds);
    });
    let connection = connection_rx.await??;
    let answer = answer_rx.await??;
    let ports = ports_rx.next().await.ok_or_else(|| anyhow!("Couldn't bind ports"))?;
    let video_process = stream::stream_video(monitor_bounds, ports.0)?;
    let remote = RemoteConnection {
        connection,
        stop_notifier: stop_notifier_out,
        video_process,
    };
    Ok((remote, answer))
}

fn start_pion_server_inner(
    connection: PeerConnection,
    ports_tx: mpsc::UnboundedSender<(u16, u16)>,
    monitor_bounds: MonitorBounds,
) {
    let (_ports_closure, ports_callback, ports_callback_user_data) = make_c_closure!(move |video: u16, audio: u16| {
        ports_tx.unbounded_send((video, audio)).unwrap();
    });

    let mut enigo = Enigo::new();
    let (_input_closure, input_callback, input_callback_user_data) =
        make_c_closure!(move |input_packet: *mut c_void, input_packet_len: usize| {
            let input_packet_raw = unsafe { std::slice::from_raw_parts(input_packet as *const u8, input_packet_len) };
            match bson::from_slice::<InputPacket>(input_packet_raw).context("Failed to deserialize packet") {
                Ok(packet) => input::handle_input(&mut enigo, monitor_bounds, packet),
                Err(err) => eprintln!("{:?}", err),
            }
        });
    unsafe {
        pion::hammer_rtp2rtc_start(
            connection,
            ports_callback,
            ports_callback_user_data,
            input_callback,
            input_callback_user_data,
        );
    }
}
