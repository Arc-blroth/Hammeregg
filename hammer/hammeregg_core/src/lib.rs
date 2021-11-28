//! # Hammeregg Core
//!
//! Core data structures for Hammeregg's
//! initial signalling handshake and it's
//! live stream packets.
//!
//! All data structures in this crate are
//! expected to be serialized with 🅱️son.
//  Maintainers: keep this file synchronized with
//  egg/src/hammeregg_core.ts

use std::error::Error;
use std::fmt::{Display, Formatter};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize, Serializer};
use tungstenite::Message;
use validator::{Validate, ValidationError};

/// The default port for Hammeregg signalling.
pub const DEFAULT_HAMMEREGG_PORT: u16 = 7269;

/// Magic number included in the header of
/// an [`InitPacket`], equal to the binary
/// representation of "🔨🥚" in UTF-8.
pub const MAGIC: i64 = 0x_F0_9F_94_A8_F0_9F_A5_9A_u64 as i64;

// Protocol Versions
/// Version 1.0
pub const VERSION_1_0: u32 = 0x_00_01__00_00;

/// A wrapper around a String that implements Error.
#[derive(Serialize, Deserialize, Debug)]
#[repr(transparent)]
#[serde(transparent)]
pub struct ErrorMsg(pub String);

impl Display for ErrorMsg {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.serialize_str(self.0.as_str())
    }
}

impl Error for ErrorMsg {}

/// The body of the various packet types sent over
/// the signalling server channel. Both the
/// [`HomeInit`] and [`RemoteInit`] packets must
/// also be wrapped in an [`HandshakeInitPacket`].
/// All other packet types consist of just this
/// enum.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HandshakePacket {
    HomeInit {
        home_name: String,
    },
    HomeInitResponse {
        response: Result<(), ErrorMsg>,
    },
    RemoteInit {
        home_name: String,
    },
    RemoteInitResponse {
        response: Result<(), ErrorMsg>,
    },
    RemoteOffer {
        peer: u32,
        key: Vec<u8>,
        iv: Vec<u8>,
        payload: Vec<u8>,
    },
    HomeAnswerSuccess {
        peer: u32,
        key: Vec<u8>,
        iv: Vec<u8>,
        payload: Vec<u8>,
    },
    HomeAnswerFailure {
        peer: u32,
        error: String,
    },
}

/// Initial handshake packet, sent by both the home
/// and remote computers to the signalling server as
/// the first packet sent. Home computers should send
/// an inner packet of type [`HomeInit`], and remote
/// computers should send an inner packet of type
/// [`RemoteInit`].
#[derive(Serialize, Deserialize, Validate)]
pub struct HandshakeInitPacket {
    #[validate(custom = "HandshakeInitPacket::validate_magic")]
    magic: i64,
    #[validate(custom = "HandshakeInitPacket::validate_version")]
    pub version: u32,
    #[validate(custom = "HandshakeInitPacket::validate_packet")]
    pub packet: HandshakePacket,
}

impl HandshakeInitPacket {
    pub fn new(version: u32, packet: HandshakePacket) -> Result<Self> {
        let new = Self {
            magic: MAGIC,
            version,
            packet,
        };
        new.validate()?;
        Ok(new)
    }

    /// Validates that the magic number is correct
    /// in a [`HandshakeInitPacket`].
    #[inline]
    fn validate_magic(magic: i64) -> Result<(), ValidationError> {
        if magic == MAGIC {
            Ok(())
        } else {
            Err(ValidationError::new("Invalid magic number!"))
        }
    }

    /// Validates that the version is [`VERSION_1_0`]
    /// in a [`HandshakeInitPacket`].
    #[inline]
    fn validate_version(version: u32) -> Result<(), ValidationError> {
        if version == VERSION_1_0 {
            Ok(())
        } else {
            Err(ValidationError::new("Unsupported version!"))
        }
    }

    /// Validates that the packet type is either
    /// [`HomeInit`] or [`RemoteInit`].
    #[inline]
    fn validate_packet(packet: &HandshakePacket) -> Result<(), ValidationError> {
        match packet {
            &HandshakePacket::HomeInit { .. } | &HandshakePacket::RemoteInit { .. } => Ok(()),
            _ => Err(ValidationError::new(
                "Init packet contents must be either HomeInit or RemoteInit",
            )),
        }
    }
}

/// Keyboard and mouse input packets, sent by the
/// remote computer over a WebRTC data channel.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputPacket {
    KeyDown(KeyInput),
    KeyUp(KeyInput),
    MouseDown(MouseButton),
    MouseUp(MouseButton),
    MouseMove { x: f32, y: f32 },
    MouseScroll { x: i32, y: i32 },
}

/// Keyboard input.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyInput {
    SpecialKey(SpecialKeyInput),
    AlphaKey(char),
    RawKey(u16),
}

/// Mouse buttons.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// "Special" input keys, based on the keys that
/// Enigo supports.
#[derive(Serialize, Deserialize)]
pub enum SpecialKeyInput {
    Alt,
    Backspace,
    CapsLock,
    Control,
    Delete,
    DownArrow,
    End,
    Escape,
    F1,
    F10,
    F11,
    F12,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    Home,
    LeftArrow,
    Meta,
    Option,
    PageDown,
    PageUp,
    Return,
    RightArrow,
    Shift,
    Space,
    Tab,
    UpArrow,
}

/// Serializes a packet to a binary message
/// containing BSON.
pub fn serialize_packet<P>(packet: &P) -> Result<Message>
where
    P: Serialize,
{
    Ok(Message::Binary(
        bson::to_vec(packet).context("Failed to serialize packet")?,
    ))
}

/// Deserializes a packet from a binary message
/// containing BSON.
pub fn deserialize_packet<'a, P>(data: &'a Message) -> Result<P>
where
    P: Deserialize<'a>,
{
    match data {
        Message::Binary(bytes) => bson::from_slice(bytes.as_slice()).context("Failed to deserialize packet"),
        _ => Err(anyhow!("Packet must be a binary message")),
    }
}

/// Deserializes a packet from a binary message
/// containing BSON and validates the packet.
pub fn deserialize_and_validate_packet<'a, P>(data: &'a Message) -> Result<P>
where
    P: Deserialize<'a> + Validate,
{
    match data {
        Message::Binary(bytes) => {
            let packet = bson::from_slice::<P>(bytes.as_slice()).context("Failed to deserialize packet")?;
            packet.validate()?;
            Ok(packet)
        }
        _ => Err(anyhow!("Packet must be a binary message")),
    }
}
