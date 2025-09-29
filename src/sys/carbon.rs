//! Minimal bindings for the Carbon API adapted from
//! <https://github.com/wusyong/carbon-bindgen/blob/467fca5d71047050b632fbdfb41b1f14575a8499/bindings.rs>
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
