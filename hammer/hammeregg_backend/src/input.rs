use enigo::{Enigo, Key, KeyboardControllable, MouseControllable};
use hammeregg_core::{InputPacket, KeyInput, MouseButton, SpecialKeyInput};

use crate::stream::MonitorBounds;

pub fn handle_input(enigo: &mut Enigo, monitor_bounds: MonitorBounds, input: InputPacket) {
    match input {
        InputPacket::KeyDown(key) => enigo.key_down(convert_key(key)),
        InputPacket::KeyUp(key) => enigo.key_up(convert_key(key)),
        InputPacket::MouseDown(butt) => enigo.mouse_down(convert_button(butt)),
        InputPacket::MouseUp(butt) => enigo.mouse_up(convert_button(butt)),
        InputPacket::MouseMove { x, y } => {
            let actual_x = (x * monitor_bounds.w as f32).round() as i32;
            let actual_y = (y * monitor_bounds.h as f32).round() as i32;
            enigo.mouse_move_to(actual_x, actual_y);
        }
        InputPacket::MouseScroll { x, y } => {
            if x != 0 {
                enigo.mouse_scroll_x(x);
            }
            if y != 0 {
                enigo.mouse_scroll_y(y);
            }
        }
    }
}

fn convert_key(key_input: KeyInput) -> Key {
    macro_rules! convert_special_key {
        ($s:ident {$($key:ident),*$(,)?}) => {
            match $s {
                $(
                    SpecialKeyInput::$key => Key::$key,
                )*
            }
        }
    }

    match key_input {
        KeyInput::SpecialKey(s) => convert_special_key!(s {
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
        }),
        KeyInput::AlphaKey(c) => Key::Layout(c),
        KeyInput::RawKey(c) => Key::Raw(c),
    }
}

fn convert_button(butt: MouseButton) -> enigo::MouseButton {
    match butt {
        MouseButton::Left => enigo::MouseButton::Left,
        MouseButton::Middle => enigo::MouseButton::Middle,
        MouseButton::Right => enigo::MouseButton::Right,
    }
}
