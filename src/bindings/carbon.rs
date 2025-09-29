//! Minimal bindings for the Carbon API adapted from
//! <https://github.com/wusyong/carbon-bindgen/blob/467fca5d71047050b632fbdfb41b1f14575a8499/bindings.rs>
use keyboard_types::Code;
use std::ffi::{c_int, c_uint, c_ulong, c_void};

pub const NO_ERR: c_int = 0;
pub const EVENT_HOTKEY_PRESSED: c_uint = 5;
pub const EVENT_PARAM_DIRECT_OBJECT: c_uint = 757935405;
pub const EVENT_HOT_KEY_ID: c_uint = 1751869796;
pub const EVENT_CLASS_KEYBOARD: c_uint = 1801812322;

#[derive(Debug, Copy, Clone)]
pub enum __EventRef {}
pub type EventRef = *mut __EventRef;

#[derive(Debug, Copy, Clone)]
pub enum __EventHandlerRef {}
pub type EventHandlerRef = *mut __EventHandlerRef;

#[derive(Debug, Copy, Clone)]
pub enum __EventHandlerCallRef {}
pub type EventHandlerCallRef = *mut __EventHandlerCallRef;

#[derive(Debug, Copy, Clone)]
pub enum __EventTargetRef {}
pub type EventTargetRef = *mut __EventTargetRef;

pub type EventHandlerFn = unsafe extern "C" fn(
    handler_call_ref: EventHandlerCallRef,
    evt: EventRef,
    user_data: *mut c_void,
) -> c_int;

#[derive(Debug, Copy, Clone)]
pub enum __EventHotKeyRef {}
pub type EventHotKeyRef = *mut __EventHotKeyRef;

#[repr(C, packed(2))]
#[derive(Debug, Copy, Clone)]
pub struct EventHotKeyID {
    pub signature: c_uint,
    pub id: c_uint,
}

#[repr(C, packed(2))]
#[derive(Debug, Copy, Clone)]
pub struct EventTypeSpec {
    pub event_class: c_uint,
    pub event_kind: c_uint,
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    pub fn GetEventParameter(
        evt: EventRef,
        name: c_uint,
        desired_type: c_uint,
        actual_type: *mut c_uint,
        buffer_size: c_ulong,
        actual_size: *mut c_ulong,
        data: *mut c_void,
    ) -> c_int;

    pub fn GetEventKind(evt: EventRef) -> c_uint;

    pub fn GetApplicationEventTarget() -> EventTargetRef;

    pub fn InstallEventHandler(
        target: EventTargetRef,
        handler: Option<EventHandlerFn>,
        num_types: c_ulong,
        list: *const EventTypeSpec,
        user_data: *mut c_void,
        handler_ref: *mut EventHandlerRef,
    ) -> c_int;

    pub fn RegisterEventHotKey(
        hot_key_code: c_uint,
        hot_key_modifiers: c_uint,
        hot_key_id: EventHotKeyID,
        target: EventTargetRef,
        options: c_uint,
        hot_key_ref: *mut EventHotKeyRef,
    ) -> c_int;

}

// can be found in https://github.com/phracker/MacOSX-SDKs/blob/master/MacOSX10.6.sdk/System/Library/Frameworks/Carbon.framework/Versions/A/Frameworks/HIToolbox.framework/Versions/A/Headers/Events.h
pub(crate) fn key_to_scancode(code: Code) -> Option<u32> {
    match code {
        Code::KeyA => Some(0x00),
        Code::KeyS => Some(0x01),
        Code::KeyD => Some(0x02),
        Code::KeyF => Some(0x03),
        Code::KeyH => Some(0x04),
        Code::KeyG => Some(0x05),
        Code::KeyZ => Some(0x06),
        Code::KeyX => Some(0x07),
        Code::KeyC => Some(0x08),
        Code::KeyV => Some(0x09),
        Code::KeyB => Some(0x0b),
        Code::KeyQ => Some(0x0c),
        Code::KeyW => Some(0x0d),
        Code::KeyE => Some(0x0e),
        Code::KeyR => Some(0x0f),
        Code::KeyY => Some(0x10),
        Code::KeyT => Some(0x11),
        Code::Digit1 => Some(0x12),
        Code::Digit2 => Some(0x13),
        Code::Digit3 => Some(0x14),
        Code::Digit4 => Some(0x15),
        Code::Digit6 => Some(0x16),
        Code::Digit5 => Some(0x17),
        Code::Equal => Some(0x18),
        Code::Digit9 => Some(0x19),
        Code::Digit7 => Some(0x1a),
        Code::Minus => Some(0x1b),
        Code::Digit8 => Some(0x1c),
        Code::Digit0 => Some(0x1d),
        Code::BracketRight => Some(0x1e),
        Code::KeyO => Some(0x1f),
        Code::KeyU => Some(0x20),
        Code::BracketLeft => Some(0x21),
        Code::KeyI => Some(0x22),
        Code::KeyP => Some(0x23),
        Code::Enter => Some(0x24),
        Code::KeyL => Some(0x25),
        Code::KeyJ => Some(0x26),
        Code::Quote => Some(0x27),
        Code::KeyK => Some(0x28),
        Code::Semicolon => Some(0x29),
        Code::Backslash => Some(0x2a),
        Code::Comma => Some(0x2b),
        Code::Slash => Some(0x2c),
        Code::KeyN => Some(0x2d),
        Code::KeyM => Some(0x2e),
        Code::Period => Some(0x2f),
        Code::Tab => Some(0x30),
        Code::Space => Some(0x31),
        Code::Backquote => Some(0x32),
        Code::Backspace => Some(0x33),
        Code::Escape => Some(0x35),
        Code::F17 => Some(0x40),
        Code::NumpadDecimal => Some(0x41),
        Code::NumpadMultiply => Some(0x43),
        Code::NumpadAdd => Some(0x45),
        Code::NumLock => Some(0x47),
        Code::AudioVolumeUp => Some(0x48),
        Code::AudioVolumeDown => Some(0x49),
        Code::AudioVolumeMute => Some(0x4a),
        Code::NumpadDivide => Some(0x4b),
        Code::NumpadEnter => Some(0x4c),
        Code::NumpadSubtract => Some(0x4e),
        Code::F18 => Some(0x4f),
        Code::F19 => Some(0x50),
        Code::NumpadEqual => Some(0x51),
        Code::Numpad0 => Some(0x52),
        Code::Numpad1 => Some(0x53),
        Code::Numpad2 => Some(0x54),
        Code::Numpad3 => Some(0x55),
        Code::Numpad4 => Some(0x56),
        Code::Numpad5 => Some(0x57),
        Code::Numpad6 => Some(0x58),
        Code::Numpad7 => Some(0x59),
        Code::F20 => Some(0x5a),
        Code::Numpad8 => Some(0x5b),
        Code::Numpad9 => Some(0x5c),
        Code::F5 => Some(0x60),
        Code::F6 => Some(0x61),
        Code::F7 => Some(0x62),
        Code::F3 => Some(0x63),
        Code::F8 => Some(0x64),
        Code::F9 => Some(0x65),
        Code::F11 => Some(0x67),
        Code::F13 => Some(0x69),
        Code::F16 => Some(0x6a),
        Code::F14 => Some(0x6b),
        Code::F10 => Some(0x6d),
        Code::F12 => Some(0x6f),
        Code::F15 => Some(0x71),
        Code::Insert => Some(0x72),
        Code::Home => Some(0x73),
        Code::PageUp => Some(0x74),
        Code::Delete => Some(0x75),
        Code::F4 => Some(0x76),
        Code::End => Some(0x77),
        Code::F2 => Some(0x78),
        Code::PageDown => Some(0x79),
        Code::F1 => Some(0x7a),
        Code::ArrowLeft => Some(0x7b),
        Code::ArrowRight => Some(0x7c),
        Code::ArrowDown => Some(0x7d),
        Code::ArrowUp => Some(0x7e),
        Code::CapsLock => Some(0x39),
        Code::PrintScreen => Some(0x46),
        _ => None,
    }
}

pub(crate) fn scancode_to_key(scancode: u32) -> Option<Code> {
    match scancode {
        0x00 => Some(Code::KeyA),
        0x01 => Some(Code::KeyS),
        0x02 => Some(Code::KeyD),
        0x03 => Some(Code::KeyF),
        0x04 => Some(Code::KeyH),
        0x05 => Some(Code::KeyG),
        0x06 => Some(Code::KeyZ),
        0x07 => Some(Code::KeyX),
        0x08 => Some(Code::KeyC),
        0x09 => Some(Code::KeyV),
        0x0b => Some(Code::KeyB),
        0x0c => Some(Code::KeyQ),
        0x0d => Some(Code::KeyW),
        0x0e => Some(Code::KeyE),
        0x0f => Some(Code::KeyR),
        0x10 => Some(Code::KeyY),
        0x11 => Some(Code::KeyT),
        0x12 => Some(Code::Digit1),
        0x13 => Some(Code::Digit2),
        0x14 => Some(Code::Digit3),
        0x15 => Some(Code::Digit4),
        0x16 => Some(Code::Digit6),
        0x17 => Some(Code::Digit5),
        0x18 => Some(Code::Equal),
        0x19 => Some(Code::Digit9),
        0x1a => Some(Code::Digit7),
        0x1b => Some(Code::Minus),
        0x1c => Some(Code::Digit8),
        0x1d => Some(Code::Digit0),
        0x1e => Some(Code::BracketRight),
        0x1f => Some(Code::KeyO),
        0x20 => Some(Code::KeyU),
        0x21 => Some(Code::BracketLeft),
        0x22 => Some(Code::KeyI),
        0x23 => Some(Code::KeyP),
        0x24 => Some(Code::Enter),
        0x25 => Some(Code::KeyL),
        0x26 => Some(Code::KeyJ),
        0x27 => Some(Code::Quote),
        0x28 => Some(Code::KeyK),
        0x29 => Some(Code::Semicolon),
        0x2a => Some(Code::Backslash),
        0x2b => Some(Code::Comma),
        0x2c => Some(Code::Slash),
        0x2d => Some(Code::KeyN),
        0x2e => Some(Code::KeyM),
        0x2f => Some(Code::Period),
        0x30 => Some(Code::Tab),
        0x31 => Some(Code::Space),
        0x32 => Some(Code::Backquote),
        0x33 => Some(Code::Backspace),
        0x35 => Some(Code::Escape),
        0x40 => Some(Code::F17),
        0x41 => Some(Code::NumpadDecimal),
        0x43 => Some(Code::NumpadMultiply),
        0x45 => Some(Code::NumpadAdd),
        0x47 => Some(Code::NumLock),
        0x48 => Some(Code::AudioVolumeUp),
        0x49 => Some(Code::AudioVolumeDown),
        0x4a => Some(Code::AudioVolumeMute),
        0x4b => Some(Code::NumpadDivide),
        0x4c => Some(Code::NumpadEnter),
        0x4e => Some(Code::NumpadSubtract),
        0x4f => Some(Code::F18),
        0x50 => Some(Code::F19),
        0x51 => Some(Code::NumpadEqual),
        0x52 => Some(Code::Numpad0),
        0x53 => Some(Code::Numpad1),
        0x54 => Some(Code::Numpad2),
        0x55 => Some(Code::Numpad3),
        0x56 => Some(Code::Numpad4),
        0x57 => Some(Code::Numpad5),
        0x58 => Some(Code::Numpad6),
        0x59 => Some(Code::Numpad7),
        0x5a => Some(Code::F20),
        0x5b => Some(Code::Numpad8),
        0x5c => Some(Code::Numpad9),
        0x60 => Some(Code::F5),
        0x61 => Some(Code::F6),
        0x62 => Some(Code::F7),
        0x63 => Some(Code::F3),
        0x64 => Some(Code::F8),
        0x65 => Some(Code::F9),
        0x67 => Some(Code::F11),
        0x69 => Some(Code::F13),
        0x6a => Some(Code::F16),
        0x6b => Some(Code::F14),
        0x6d => Some(Code::F10),
        0x6f => Some(Code::F12),
        0x71 => Some(Code::F15),
        0x72 => Some(Code::Insert),
        0x73 => Some(Code::Home),
        0x74 => Some(Code::PageUp),
        0x75 => Some(Code::Delete),
        0x76 => Some(Code::F4),
        0x77 => Some(Code::End),
        0x78 => Some(Code::F2),
        0x79 => Some(Code::PageDown),
        0x7a => Some(Code::F1),
        0x7b => Some(Code::ArrowLeft),
        0x7c => Some(Code::ArrowRight),
        0x7d => Some(Code::ArrowDown),
        0x7e => Some(Code::ArrowUp),
        0x39 => Some(Code::CapsLock),
        0x46 => Some(Code::PrintScreen),
        _ => None,
    }
}
