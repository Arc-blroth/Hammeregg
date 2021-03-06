// Core data structures for Hammeregg's
// initial signalling handshake and it's
// live stream packets.
//
// Maintainers: keep this file synchronized with
// hammer/hammeregg_core/src/lib.rs

import * as BSON from "bson"

/** The default port for Hammeregg signalling. */
export const DEFAULT_HAMMEREGG_PORT: number = 7269

/**
 * Magic number included in the header of
 * an `InitPacket`, equal to the binary
 * representation of "🔨🥚" in UTF-8.
 */
export const MAGIC: BSON.Long = BSON.Long.fromBytes(
    [0xf0, 0x9f, 0x94, 0xa8, 0xf0, 0x9f, 0xa5, 0x9a],
    true
)

/** Version 1.0 */
export const VERSION_1_0: number = 0x00_01_00_00

export interface ResultOk {
    Ok: null
}

export interface ResultErr {
    Err: string
}

export type Result = ResultOk | ResultErr

export enum HandshakePacketType {
    HOME_INIT = "HomeInit",
    HOME_INIT_RESPONSE = "HomeInitResponse",
    REMOTE_INIT = "RemoteInit",
    REMOTE_INIT_RESPONSE = "RemoteInitResponse",
    REMOTE_OFFER = "RemoteOffer",
    HOME_ANSWER_SUCCESS = "HomeAnswerSuccess",
    HOME_ANSWER_FAILURE = "HomeAnswerFailure",
}

export interface HomeInitHandshakePacket {
    type: HandshakePacketType.HOME_INIT
    home_name: string
}

export interface HomeInitResponseHandshakePacket {
    type: HandshakePacketType.HOME_INIT_RESPONSE
    response: Result
}

export interface RemoteInitHandshakePacket {
    type: HandshakePacketType.REMOTE_INIT
    home_name: string
}

export interface RemoteInitResponseHandshakePacket {
    type: HandshakePacketType.REMOTE_INIT_RESPONSE
    response: Result
}

export interface RemoteOfferHandshakePacket {
    type: HandshakePacketType.REMOTE_OFFER
    peer: number
    key: Array<number>
    iv: Array<number>
    payload: Array<number>
}

export interface HomeAnswerSuccessHandshakePacket {
    type: HandshakePacketType.HOME_ANSWER_SUCCESS
    peer: number
    key: Array<number>
    iv: Array<number>
    payload: Array<number>
}

export interface HomeAnswerFailureHandshakePacket {
    type: HandshakePacketType.HOME_ANSWER_FAILURE
    peer: number
    error: string
}

/**
 * The body of the various packet types sent over
 * the signalling server channel. Both the
 * `HomeInit` and `RemoteInit` packets must
 * also be wrapped in an `HandshakeInitPacket`.
 * All other packet types consist of just this
 * enum.
 */
export type HandshakePacket =
    | HomeInitHandshakePacket
    | HomeInitResponseHandshakePacket
    | RemoteInitHandshakePacket
    | RemoteInitResponseHandshakePacket
    | RemoteOfferHandshakePacket
    | HomeAnswerSuccessHandshakePacket
    | HomeAnswerFailureHandshakePacket

/**
 * Initial handshake packet, sent by both the home
 * and remote computers to the signalling server as
 * the first packet sent. Home computers should send
 * an inner packet of type `HomeInit`, and remote
 * computers should send an inner packet of type
 * `RemoteInit`.
 */
export interface HandshakeInitPacket {
    magic: BSON.Long
    version: number
    packet: HandshakePacket
}

/**
 * Keyboard and mouse input packets, sent by the
 * remote computer over a WebRTC data channel.
 */
export type Input =
    | { key_down: KeyInput }
    | { key_up: KeyInput }
    | { mouse_down: MouseButton }
    | { mouse_up: MouseButton }
    | { mouse_move: { x: number; y: number } }
    | { mouse_scroll: { x: number; y: number } }

/**
 * Keyboard input.
 */
export type KeyInput =
    | { special_key: SpecialKeyInput }
    | { alpha_key: string }
    | { raw_key: number }

/**
 * Mouse buttons.
 */
export enum MouseButton {
    Left = "Left",
    Middle = "Middle",
    Right = "Right",
}

/**
 * "Special" input keys, based on the keys that
 * Enigo supports.
 */
export enum SpecialKeyInput {
    Alt = "Alt",
    Backspace = "Backspace",
    CapsLock = "CapsLock",
    Control = "Control",
    Delete = "Delete",
    DownArrow = "DownArrow",
    End = "End",
    Escape = "Escape",
    F1 = "F1",
    F10 = "F10",
    F11 = "F11",
    F12 = "F12",
    F2 = "F2",
    F3 = "F3",
    F4 = "F4",
    F5 = "F5",
    F6 = "F6",
    F7 = "F7",
    F8 = "F8",
    F9 = "F9",
    Home = "Home",
    LeftArrow = "LeftArrow",
    Meta = "Meta",
    Option = "Option",
    PageDown = "PageDown",
    PageUp = "PageUp",
    Return = "Return",
    RightArrow = "RightArrow",
    Shift = "Shift",
    Space = "Space",
    Tab = "Tab",
    UpArrow = "UpArrow",
}
