//! Hotkey handling adapted from <https://github.com/tauri-apps/global-hotkey>
use crate::{
    bindings::carbon::{
        EVENT_CLASS_KEYBOARD, EVENT_HOT_KEY_ID, EVENT_HOTKEY_PRESSED, EVENT_PARAM_DIRECT_OBJECT,
        EventHandlerCallRef, EventHandlerRef, EventHotKeyID, EventHotKeyRef, EventRef,
        EventTypeSpec, GetApplicationEventTarget, GetEventKind, GetEventParameter,
        InstallEventHandler, NO_ERR, RegisterEventHotKey, key_to_scancode, scancode_to_key,
    },
    conn::OsxConn,
    event::Event,
    sys::EVENT_SENDER,
};
use keyboard_types::{Code, Modifiers};
use penrose::{
    Error, Result,
    core::bindings::{KeyBindings, KeyEventHandler},
    custom_error,
};
use std::{
    collections::HashMap,
    ffi::{c_int, c_ulong, c_void},
    io,
    mem::{size_of, zeroed},
    ptr::null_mut,
};
use tracing::{debug, warn};

mod carbon;

// see https://github.com/soffes/HotKey/blob/c13662730cb5bc28de4a799854bbb018a90649bf/Sources/HotKey/HotKeysController.swift#L27
const fn sig() -> u32 {
    let mut sig = 'p' as u32;
    sig = (sig << 8) + 'n' as u32;
    sig = (sig << 8) + 'r' as u32;
    sig = (sig << 8) + 's' as u32;

    sig
}

#[derive(Debug)]
pub struct KeyListener {
    _evt_handler: EventHandlerRef,
    hotkey_refs: Vec<EventHotKeyRef>,
}

unsafe impl Send for KeyListener {}
unsafe impl Sync for KeyListener {}

impl KeyListener {
    pub fn try_new() -> Result<Self> {
        let evt_types = [EventTypeSpec {
            event_class: EVENT_CLASS_KEYBOARD,
            event_kind: EVENT_HOTKEY_PRESSED,
        }];

        let evt_handler = unsafe {
            let mut evt_handler: EventHandlerRef = zeroed();
            let res = InstallEventHandler(
                GetApplicationEventTarget(),
                Some(event_handler),
                evt_types.len() as c_ulong,
                evt_types.as_ptr(),
                null_mut(),
                &mut evt_handler,
            );

            if res != NO_ERR {
                return Err(custom_error!(
                    "failed to create key listener {}",
                    io::Error::last_os_error()
                ));
            }

            evt_handler
        };

        Ok(Self {
            _evt_handler: evt_handler,
            hotkey_refs: Vec::new(),
        })
    }

    // Register a new hotkey and return its ID.
    pub(crate) fn register(&mut self, k: &HotKey) -> Result<u32> {
        debug!(?k, "registering hotkey");
        let scan_code = k.try_scancode()?;
        let mod_mask = k.mod_mask();
        let id = k.id();

        let hotkey_id = EventHotKeyID {
            id,
            signature: sig(),
        };

        let hotkey_ref = unsafe {
            let mut hotkey_ref: EventHotKeyRef = zeroed();
            let res = RegisterEventHotKey(
                scan_code,
                mod_mask,
                hotkey_id,
                GetApplicationEventTarget(),
                0,
                &mut hotkey_ref,
            );

            if res != NO_ERR {
                return Err(custom_error!("RegisterEventHotKey failed for {}", k.key));
            }

            hotkey_ref
        };

        self.hotkey_refs.push(hotkey_ref);

        Ok(id)
    }
}

// Extract the hotkey ID from the event and hand back to the Conn for processing.
unsafe extern "C" fn event_handler(
    _next_handler: EventHandlerCallRef,
    evt: EventRef,
    _data: *mut c_void,
) -> c_int {
    unsafe {
        let mut event_hotkey: EventHotKeyID = zeroed();

        let res = GetEventParameter(
            evt,
            EVENT_PARAM_DIRECT_OBJECT,
            EVENT_HOT_KEY_ID,
            null_mut(),
            size_of::<EventHotKeyID>() as _,
            null_mut(),
            &mut event_hotkey as *mut _ as *mut _,
        );

        if res == NO_ERR {
            match GetEventKind(evt) {
                EVENT_HOTKEY_PRESSED => {
                    let k = match HotKey::try_from_id(event_hotkey.id) {
                        Ok(k) => k,
                        Err(e) => {
                            warn!("{e}");
                            return NO_ERR;
                        }
                    };

                    _ = EVENT_SENDER.wait().send(Event::KeyPress { k });
                }

                kind => warn!(%kind, "unknown hotkey event kind"),
            };
        }
    }

    NO_ERR
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HotKey {
    pub mods: Modifiers,
    pub key: Code,
}

impl HotKey {
    fn id(&self) -> u32 {
        (self.mods.bits() << 16) | self.try_scancode().unwrap()
    }

    fn mod_mask(&self) -> u32 {
        let mut mod_mask: u32 = 0;

        if self.mods.shift() {
            mod_mask |= 512;
        }
        if self.mods.meta() {
            mod_mask |= 256;
        }
        if self.mods.alt() {
            mod_mask |= 2048;
        }
        if self.mods.ctrl() {
            mod_mask |= 4096;
        }

        mod_mask
    }

    fn try_scancode(&self) -> Result<u32> {
        key_to_scancode(self.key).ok_or_else(|| custom_error!("Unknown scancode for {}", self.key))
    }

    fn try_from_id(id: u32) -> Result<Self> {
        let key = scancode_to_key(id & 0x00FF)
            .ok_or_else(|| custom_error!("invalid scancode {}", id & 0x00FF))?;
        let mods = Modifiers::from_bits(id >> 16)
            .ok_or_else(|| custom_error!("invalid modifiers {}", id & 0xFF00))?;

        Ok(Self { mods, key })
    }
}

impl TryFrom<&str> for HotKey {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self> {
        let mut parts: Vec<&str> = s.split('-').collect();
        let name = parts.remove(parts.len() - 1);
        let key = try_parse_key(name).ok_or_else(|| custom_error!("unknown key {:?}", name))?;

        let mods = parts
            .iter()
            .try_fold(Modifiers::empty(), |mods, &s| match s {
                "C" => Ok(mods | Modifiers::CONTROL),
                "A" => Ok(mods | Modifiers::ALT),
                "S" => Ok(mods | Modifiers::SHIFT),
                "M" => Ok(mods | Modifiers::META),
                _ => Err(Error::UnknownModifier { name: s.to_owned() }),
            })?;

        Ok(Self { mods, key })
    }
}

pub fn try_parse_key_bindings(
    raw: HashMap<impl AsRef<str>, Box<dyn KeyEventHandler<OsxConn>>>,
) -> Result<KeyBindings<OsxConn>> {
    raw.into_iter()
        .map(|(s, handler)| HotKey::try_from(s.as_ref()).map(|hk| (hk, handler)))
        .collect()
}

fn try_parse_key(key: &str) -> Option<Code> {
    use Code::*;
    match key.to_uppercase().as_str() {
        "BACKQUOTE" | "`" => Some(Backquote),
        "BACKSLASH" | "\\" => Some(Backslash),
        "BRACKETLEFT" | "[" => Some(BracketLeft),
        "BRACKETRIGHT" | "]" => Some(BracketRight),
        "PAUSE" | "PAUSEBREAK" => Some(Pause),
        "COMMA" | "," => Some(Comma),
        "0" => Some(Digit0),
        "1" => Some(Digit1),
        "2" => Some(Digit2),
        "3" => Some(Digit3),
        "4" => Some(Digit4),
        "5" => Some(Digit5),
        "6" => Some(Digit6),
        "7" => Some(Digit7),
        "8" => Some(Digit8),
        "9" => Some(Digit9),
        "EQUALS" | "=" => Some(Equal),
        "A" => Some(KeyA),
        "B" => Some(KeyB),
        "C" => Some(KeyC),
        "D" => Some(KeyD),
        "E" => Some(KeyE),
        "F" => Some(KeyF),
        "G" => Some(KeyG),
        "H" => Some(KeyH),
        "I" => Some(KeyI),
        "J" => Some(KeyJ),
        "K" => Some(KeyK),
        "L" => Some(KeyL),
        "M" => Some(KeyM),
        "N" => Some(KeyN),
        "O" => Some(KeyO),
        "P" => Some(KeyP),
        "Q" => Some(KeyQ),
        "R" => Some(KeyR),
        "S" => Some(KeyS),
        "T" => Some(KeyT),
        "U" => Some(KeyU),
        "V" => Some(KeyV),
        "W" => Some(KeyW),
        "X" => Some(KeyX),
        "Y" => Some(KeyY),
        "Z" => Some(KeyZ),
        "MINUS" | "-" => Some(Minus),
        "PERIOD" | "." => Some(Period),
        "QUOTE" | "'" => Some(Quote),
        "SEMICOLON" | ";" => Some(Semicolon),
        "SLASH" | "/" => Some(Slash),
        "BACKSPACE" => Some(Backspace),
        "CAPSLOCK" => Some(CapsLock),
        "ENTER" => Some(Enter),
        "SPACE" => Some(Space),
        "TAB" => Some(Tab),
        "DELETE" => Some(Delete),
        "END" => Some(End),
        "HOME" => Some(Home),
        "INSERT" => Some(Insert),
        "PAGEDOWN" => Some(PageDown),
        "PAGEUP" => Some(PageUp),
        "PRINTSCREEN" => Some(PrintScreen),
        "SCROLLLOCK" => Some(ScrollLock),
        "DOWN" => Some(ArrowDown),
        "LEFT" => Some(ArrowLeft),
        "RIGHT" => Some(ArrowRight),
        "UP" => Some(ArrowUp),
        "NUMLOCK" => Some(NumLock),
        "NUMPAD0" | "NUM0" => Some(Numpad0),
        "NUMPAD1" | "NUM1" => Some(Numpad1),
        "NUMPAD2" | "NUM2" => Some(Numpad2),
        "NUMPAD3" | "NUM3" => Some(Numpad3),
        "NUMPAD4" | "NUM4" => Some(Numpad4),
        "NUMPAD5" | "NUM5" => Some(Numpad5),
        "NUMPAD6" | "NUM6" => Some(Numpad6),
        "NUMPAD7" | "NUM7" => Some(Numpad7),
        "NUMPAD8" | "NUM8" => Some(Numpad8),
        "NUMPAD9" | "NUM9" => Some(Numpad9),
        "NUMPADADD" | "NUMADD" | "NUMPADPLUS" | "NUMPLUS" => Some(NumpadAdd),
        "NUMPADDECIMAL" | "NUMDECIMAL" => Some(NumpadDecimal),
        "NUMPADDIVIDE" | "NUMDIVIDE" => Some(NumpadDivide),
        "NUMPADENTER" | "NUMENTER" => Some(NumpadEnter),
        "NUMPADEQUAL" | "NUMEQUAL" => Some(NumpadEqual),
        "NUMPADMULTIPLY" | "NUMMULTIPLY" => Some(NumpadMultiply),
        "NUMPADSUBTRACT" | "NUMSUBTRACT" => Some(NumpadSubtract),
        "ESCAPE" | "ESC" => Some(Escape),
        "F1" => Some(F1),
        "F2" => Some(F2),
        "F3" => Some(F3),
        "F4" => Some(F4),
        "F5" => Some(F5),
        "F6" => Some(F6),
        "F7" => Some(F7),
        "F8" => Some(F8),
        "F9" => Some(F9),
        "F10" => Some(F10),
        "F11" => Some(F11),
        "F12" => Some(F12),
        "F13" => Some(F13),
        "F14" => Some(F14),
        "F15" => Some(F15),
        "F16" => Some(F16),
        "F17" => Some(F17),
        "F18" => Some(F18),
        "F19" => Some(F19),
        "F20" => Some(F20),
        "F21" => Some(F21),
        "F22" => Some(F22),
        "F23" => Some(F23),
        "F24" => Some(F24),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple_test_case::test_case;

    pub(crate) const SUPPORTED_CODES: [Code; 105] = [
        Code::KeyA,
        Code::KeyS,
        Code::KeyD,
        Code::KeyF,
        Code::KeyH,
        Code::KeyG,
        Code::KeyZ,
        Code::KeyX,
        Code::KeyC,
        Code::KeyV,
        Code::KeyB,
        Code::KeyQ,
        Code::KeyW,
        Code::KeyE,
        Code::KeyR,
        Code::KeyY,
        Code::KeyT,
        Code::Digit1,
        Code::Digit2,
        Code::Digit3,
        Code::Digit4,
        Code::Digit6,
        Code::Digit5,
        Code::Equal,
        Code::Digit9,
        Code::Digit7,
        Code::Minus,
        Code::Digit8,
        Code::Digit0,
        Code::BracketRight,
        Code::KeyO,
        Code::KeyU,
        Code::BracketLeft,
        Code::KeyI,
        Code::KeyP,
        Code::Enter,
        Code::KeyL,
        Code::KeyJ,
        Code::Quote,
        Code::KeyK,
        Code::Semicolon,
        Code::Backslash,
        Code::Comma,
        Code::Slash,
        Code::KeyN,
        Code::KeyM,
        Code::Period,
        Code::Tab,
        Code::Space,
        Code::Backquote,
        Code::Backspace,
        Code::Escape,
        Code::F17,
        Code::NumpadDecimal,
        Code::NumpadMultiply,
        Code::NumpadAdd,
        Code::NumLock,
        Code::AudioVolumeUp,
        Code::AudioVolumeDown,
        Code::AudioVolumeMute,
        Code::NumpadDivide,
        Code::NumpadEnter,
        Code::NumpadSubtract,
        Code::F18,
        Code::F19,
        Code::NumpadEqual,
        Code::Numpad0,
        Code::Numpad1,
        Code::Numpad2,
        Code::Numpad3,
        Code::Numpad4,
        Code::Numpad5,
        Code::Numpad6,
        Code::Numpad7,
        Code::F20,
        Code::Numpad8,
        Code::Numpad9,
        Code::F5,
        Code::F6,
        Code::F7,
        Code::F3,
        Code::F8,
        Code::F9,
        Code::F11,
        Code::F13,
        Code::F16,
        Code::F14,
        Code::F10,
        Code::F12,
        Code::F15,
        Code::Insert,
        Code::Home,
        Code::PageUp,
        Code::Delete,
        Code::F4,
        Code::End,
        Code::F2,
        Code::PageDown,
        Code::F1,
        Code::ArrowLeft,
        Code::ArrowRight,
        Code::ArrowDown,
        Code::ArrowUp,
        Code::CapsLock,
        Code::PrintScreen,
    ];

    #[test]
    fn scancode_mapping_fns_are_inverse() {
        for code in SUPPORTED_CODES.iter() {
            let code2 = scancode_to_key(key_to_scancode(*code).unwrap()).unwrap();
            assert_eq!(*code, code2);
        }
    }

    #[test_case(Modifiers::empty(); "none")]
    #[test_case(Modifiers::SHIFT; "shift")]
    #[test_case(Modifiers::ALT; "alt")]
    #[test_case(Modifiers::CONTROL; "ctrl")]
    #[test_case(Modifiers::META; "meta")]
    #[test_case(Modifiers::SHIFT | Modifiers::ALT; "shift alt")]
    #[test_case(Modifiers::SHIFT | Modifiers::CONTROL; "shift ctrl")]
    #[test_case(Modifiers::SHIFT | Modifiers::META; "shift meta")]
    #[test_case(Modifiers::ALT | Modifiers::CONTROL; "alt ctrl")]
    #[test_case(Modifiers::ALT | Modifiers::META; "alt meta")]
    #[test_case(Modifiers::CONTROL | Modifiers::META; "ctrl meta")]
    #[test_case(Modifiers::SHIFT | Modifiers::ALT | Modifiers::CONTROL; "shift alt ctrl")]
    #[test_case(Modifiers::SHIFT | Modifiers::ALT | Modifiers::META; "shift alt meta")]
    #[test_case(Modifiers::SHIFT | Modifiers::CONTROL | Modifiers::META; "shift ctrl meta")]
    #[test_case(Modifiers::ALT | Modifiers::CONTROL | Modifiers::META; "alt ctrl meta")]
    #[test_case(Modifiers::SHIFT | Modifiers::ALT | Modifiers::CONTROL | Modifiers::META; "all")]
    #[test]
    fn id_mapping_fns_are_inverse(mods: Modifiers) {
        for &key in SUPPORTED_CODES.iter() {
            let k = HotKey { mods, key };
            let id = k.id();
            let parsed = HotKey::try_from_id(id)
                .unwrap_or_else(|e| panic!("unable to parse back {k:?}: {e}"));

            assert_eq!(k, parsed);
        }
    }
}
