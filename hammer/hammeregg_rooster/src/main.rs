#![feature(try_blocks)]

use std::collections::HashMap;
use std::fmt::Display;
use std::fs::File;
use std::io::BufReader;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use futures::channel::mpsc::{unbounded, UnboundedSender};
use futures::{future, pin_mut, SinkExt, StreamExt, TryStreamExt};
use hammeregg_core::{
    deserialize_and_validate_packet, deserialize_packet, serialize_packet, ErrorMsg, HandshakeInitPacket,
    HandshakePacket, DEFAULT_HAMMEREGG_PORT,
};
use parking_lot::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig};
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

/// A wrapper around a desktop and a set of peers
/// where each peer has a unique id.
pub struct DesktopAndPeers {
    pub desktop_tx: UnboundedSender<Message>,
    id_counter: u32,
    peers: HashMap<u32, UnboundedSender<Message>>,
}

impl DesktopAndPeers {
    pub fn new(desktop_tx: UnboundedSender<Message>) -> Self {
        Self {
            desktop_tx,
            id_counter: 0,
            peers: HashMap::new(),
        }
    }

    pub fn insert_peer(&mut self, peer: UnboundedSender<Message>) -> u32 {
        let id = self.id_counter;
        self.peers.insert(id, peer);
        self.id_counter += 1;
        id
    }

    pub fn get_peer(&self, id: &u32) -> Option<&UnboundedSender<Message>> {
        self.peers.get(id)
    }

    pub fn get_peer_mut(&mut self, id: &u32) -> Option<&mut UnboundedSender<Message>> {
        self.peers.get_mut(id)
    }

    pub fn remove_peer(&mut self, id: &u32) {
        self.peers.remove(id);
    }

    pub fn peers(&self) -> &HashMap<u32, UnboundedSender<Message>> {
        &self.peers
    }
}

/// A concurrent map between home desktops' names and a tuple of
/// their sending end and their peers' sending ends.
type Desktops = Arc<Mutex<HashMap<String, DesktopAndPeers>>>;

/// A WebSocket Secure stream
type WSS = WebSocketStream<TlsStream<TcpStream>>;

#[tokio::main]
async fn main() -> Result<()> {
    let default_port = DEFAULT_HAMMEREGG_PORT.to_string();
    let matches = clap::clap_app!("Hammeregg Rooster" =>
        (about: "A signalling server implementation for Hammeregg.")
        (version: clap::crate_version!())
        (license: clap::crate_license!())
        (@arg IP: -a --addr default_value("127.0.0.1") validator(validate_ip)
            "Custom address to run Rooster on")
        (@arg PORT: -p --port default_value(default_port.as_str()) validator(validate_port)
            "Custom port to run Rooster on")
        (@arg CERTIFICATES: -c --cert required(true) takes_value(true) validator(validate_certs)
            ".crt file to trust in Rooster's TLS certificate chain")
        (@arg KEY: -k --key required(true) takes_value(true) validator(validate_key)
            ".key file to use as Rooster's server private key")
    )
    .get_matches();

    // These use `.unwrap()` since clap has already ensured that everything is valid.
    let ip = validate_ip(matches.value_of("IP").unwrap()).unwrap();
    let port = validate_port(matches.value_of("PORT").unwrap()).unwrap();
    let certs = validate_certs(matches.value_of("CERTIFICATES").unwrap()).unwrap();
    let key = validate_key(matches.value_of("KEY").unwrap()).unwrap();

    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("Invalid cert/key!")?;

    let addr = SocketAddr::new(ip, port);
    let listener = TcpListener::bind(&addr).await.context("Couldn't bind to port")?;
    let acceptor = TlsAcceptor::from(Arc::new(config));
    println!("Rooster listening at wss://{}", addr);

    let desktops = Desktops::default();

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_connection(acceptor.clone(), stream, desktops.clone()));
    }

    Ok(())
}

fn display_err<E: Display>(err: E) -> String {
    format!("{}", err)
}

fn validate_port(val: &str) -> Result<u16, String> {
    u16::from_str(val).map_err(display_err)
}

fn validate_ip(val: &str) -> Result<IpAddr, String> {
    IpAddr::from_str(val).map_err(display_err)
}

fn validate_certs(val: &str) -> Result<Vec<Certificate>, String> {
    rustls_pemfile::certs(&mut BufReader::new(File::open(val).map_err(display_err)?))
        .map(|mut certs| certs.drain(..).map(Certificate).collect())
        .map_err(display_err)
}

fn validate_key(val: &str) -> Result<PrivateKey, String> {
    let mut keys = rustls_pemfile::rsa_private_keys(&mut BufReader::new(File::open(val).map_err(display_err)?))
        .map_err(display_err)?;
    if !keys.is_empty() {
        Ok(PrivateKey(keys.swap_remove(0)))
    } else {
        Err("Key file must contain a single key".to_string())
    }
}

// Generic handler for both Desktop and Egg connections.
async fn handle_connection(acceptor: TlsAcceptor, stream: TcpStream, desktops: Desktops) {
    let res: Result<()> = try {
        let tls_stream = acceptor
            .accept(stream)
            .await
            .context("Error during the TLS handshake occurred")?;
        let mut socket = tokio_tungstenite::accept_async(tls_stream)
            .await
            .context("Error during the websocket handshake occurred")?;

        match deserialize_and_validate_packet::<HandshakeInitPacket>(
            &socket
                .next()
                .await
                .context("Handshake failed: could not read packet")??,
        )?
        .packet
        {
            HandshakePacket::HomeInit { home_name } => {
                handle_home_init(desktops, socket, home_name).await?;
            }
            HandshakePacket::RemoteInit { home_name } => {
                handle_remote_init(desktops, socket, home_name).await?;
            }
            _ => Err(anyhow!(
                "Handshake failed: client did not send a valid HandshakeInitPacket"
            ))?,
        }
    };
    if let Err(err) = res {
        eprintln!("{:?}", err);
    }
}

async fn handle_home_init(desktops: Desktops, mut socket: WSS, home_name: String) -> Result<()> {
    if desktops.lock().contains_key(&home_name) {
        // oops there's already another computer with this name
        socket
            .send(serialize_packet(&HandshakePacket::HomeInitResponse {
                response: Err(ErrorMsg("Requested desktop name was already taken".to_string())),
            })?)
            .await?;
    } else {
        // Initial handshake complete!
        socket
            .send(serialize_packet(&HandshakePacket::HomeInitResponse {
                response: Ok(()),
            })?)
            .await?;

        let (tx, rx) = unbounded();
        // Insert sender into desktop map
        desktops.lock().insert(home_name.clone(), DesktopAndPeers::new(tx));

        let (send, recv) = socket.split();

        // Listen to incoming requests to send back home
        let send_home = rx.map(Ok).forward(send);

        // Listen to incoming requests to send to peers
        let send_peer = recv
            .map(|res| res.context("Signalling failed: could not read packet"))
            .try_for_each(|packet| {
                match try {
                    match deserialize_packet::<HandshakePacket>(&packet)? {
                        HandshakePacket::HomeAnswerSuccess { peer, .. }
                        | HandshakePacket::HomeAnswerFailure { peer, .. } => {
                            let mut desktop_map = desktops.lock();
                            let maybe_peer = desktop_map.get_mut(&home_name).unwrap().get_peer_mut(&peer);
                            if let Some(peer) = maybe_peer {
                                peer.unbounded_send(packet).context("Couldn't send packet")?
                            } else {
                                // Whoops the peer no longer exists
                                Err(anyhow!("Signalling failed: peer does not exist (any longer)"))?
                            }
                        }
                        _ => Err(anyhow!(
                            "Signalling failed: did not get a HomeAnswerSuccess or HomeAnswerResponse packet"
                        ))?,
                    }
                } {
                    Ok(_) => future::ok(()),
                    Err(err) => future::err(err),
                }
            });

        pin_mut!(send_home, send_peer);
        future::select(send_home, send_peer).await;

        // Disconnect
        let mut desktop_map = desktops.lock();
        if let Some(desktop) = desktop_map.remove(&home_name) {
            desktop.peers().iter().for_each(|(_, peer)| {
                peer.close_channel();
            });
        }
    }
    Ok(())
}

async fn handle_remote_init(desktops: Desktops, mut socket: WSS, home_name: String) -> Result<()> {
    if !desktops.lock().contains_key(&home_name) {
        // oops desktop does not exist
        socket
            .send(serialize_packet(&HandshakePacket::RemoteInitResponse {
                response: Err(ErrorMsg("Requested desktop not found".to_string())),
            })?)
            .await?;
    } else {
        // Initial handshake complete!
        socket
            .send(serialize_packet(&HandshakePacket::RemoteInitResponse {
                response: Ok(()),
            })?)
            .await?;

        let (tx, rx) = unbounded();
        // Insert sender into desktop map
        let id = desktops
            .lock()
            .get_mut(&home_name)
            .context("Desktop disappeared during remote init?")?
            .insert_peer(tx);

        let (send, recv) = socket.split();

        // Listen to incoming requests to send back to the remote
        let send_remote = rx.map(Ok).forward(send);

        // Listen to incoming requests to send home
        let send_home = recv
            .map(|res| res.context("Signalling failed: could not read packet"))
            .try_for_each(|packet| {
                match try {
                    match deserialize_packet::<HandshakePacket>(&packet)? {
                        HandshakePacket::RemoteOffer { payload, .. } => {
                            let mut desktop_map = desktops.lock();
                            let desktop = desktop_map
                                .get_mut(&home_name)
                                .context("Desktop does not exist any longer")?;

                            // Since remote doesn't know their peer id we need to fill it in
                            let filled_packet = serialize_packet(&HandshakePacket::RemoteOffer { peer: id, payload })?;

                            desktop
                                .desktop_tx
                                .unbounded_send(filled_packet)
                                .context("Couldn't send packet")?;
                        }
                        _ => Err(anyhow!("Signalling failed: did not get a RemoteOffer packet"))?,
                    }
                } {
                    Ok(_) => future::ok(()),
                    Err(err) => future::err(err),
                }
            });

        pin_mut!(send_remote, send_home);
        future::select(send_remote, send_home).await;

        // Disconnect
        let mut desktop_map = desktops.lock();
        if let Some(desktop) = desktop_map.get_mut(&home_name) {
            desktop.remove_peer(&id);
        }
    }
    Ok(())
}
