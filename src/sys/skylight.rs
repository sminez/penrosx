//! Minimal bindings for the Skylight API adapted from the approaches taken in
//! <https://github.com/koekeishiya/yabai>.
use crate::{
    ROOT,
    event::{EVENT_SENDER, Event},
    sys::Pid,
};
use penrose::{Result, WinId, custom_error};
use std::{ffi::c_void, ptr, sync::LazyLock};
use tracing::warn;

static SLS_MAIN_CONN_ID: LazyLock<i32> = LazyLock::new(|| unsafe { SLSMainConnectionID() });

#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C" {
    fn _SLPSSetFrontProcessWithOptions(psn: *const Psn, wid: u32, mode: u32) -> i32;
    fn SLPSPostEventRecordTo(psn: *const Psn, bytes: *const u8) -> i32;
    fn SLSMainConnectionID() -> i32;
    fn SLSRegisterConnectionNotifyProc(
        cid: i32,
        callback: RegisterConnCallback,
        event: u32,
        data: *mut c_void,
    ) -> i32;
    fn SLSRequestNotificationsForWindows(
        cid: i32,
        window_list: *const u32,
        window_count: i32,
    ) -> i32;
}

type RegisterConnCallback = unsafe extern "C" fn(
    event: CGSEvent,
    data: *mut c_void,
    data_len: usize,
    ctx: *mut c_void,
    cid: i32,
);

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    // Deprecated in macOS 10.9?
    fn GetProcessForPID(pid: Pid, psn: *mut Psn) -> i32;
}

macro_rules! check {
    ($exp:expr) => {
        if $exp != 0 {
            return Err(custom_error!("Skylight error {err}"));
        }
    };
}

// See https://github.com/Hammerspoon/hammerspoon/issues/370#issuecomment-545545468.
pub(crate) fn make_key_window(pid: Pid, id: WinId) -> Result<()> {
    let user_generated: u32 = 512;

    // the information specified in the events below consists of the "special" category, event type, and modifiers,
    // basically synthesizing a mouse-down and up event targeted at a specific window of the application,
    // but it doesn't actually get treated as a mouse-click normally would.
    let mut event1 = [0; 256];
    event1[4] = 248;
    event1[8] = 1; // mouse down
    event1[58] = 16;
    event1[60..64].copy_from_slice(&id.to_le_bytes());
    event1[32..48].fill(255);

    let mut event2 = event1;
    event2[8] = 2; // mouse up

    let mut psn = Psn::default();
    if unsafe { GetProcessForPID(pid, &mut psn) } != 0 {
        return Err(custom_error!("unable to find PSN"));
    }

    unsafe {
        // focus the process and tell it which window should get key-focus
        check!(_SLPSSetFrontProcessWithOptions(&psn, id.0, user_generated));
        // synthesize click events to have the process update the key-window internally
        check!(SLPSPostEventRecordTo(&psn, event1.as_ptr()));
        check!(SLPSPostEventRecordTo(&psn, event2.as_ptr()));
    }

    Ok(())
}

// PSN: Process Serial Number
#[repr(C)]
#[derive(Default)]
struct Psn {
    high: u32,
    low: u32,
}

pub(crate) fn update_sls_window_notifications(it: impl Iterator<Item = WinId>) -> Result<()> {
    let win_ids: Vec<_> = it.map(|WinId(id)| id).collect();

    unsafe {
        check!(SLSRequestNotificationsForWindows(
            *SLS_MAIN_CONN_ID,
            win_ids.as_ptr(),
            win_ids.len() as i32,
        ));
    }

    Ok(())
}

pub(crate) fn register_for_sls_notifications() -> Result<()> {
    for evt in CGSEvent::HANDLED_EVENTS {
        unsafe {
            check!(SLSRegisterConnectionNotifyProc(
                *SLS_MAIN_CONN_ID,
                register_conn_callback,
                evt.0,
                ptr::null_mut(),
            ));
        };
    }

    Ok(())
}

// See https://github.com/NUIKit/CGSInternal/blob/c4f6f559d624dc1cfc2bf24c8c19dbf653317fcf/CGSEvent.h#L21
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CGSEvent(u32);
impl CGSEvent {
    const WINDOW_CLOSED: Self = Self(804);
    const WINDOW_UNHIDDEN: Self = Self(815);
    const WINDOW_HIDDEN: Self = Self(816);
    // const WINDOW_CREATED: Self = Self(1325);
    const WINDOW_DESTROYED: Self = Self(1326);

    const HANDLED_EVENTS: [Self; 4] = [
        Self::WINDOW_CLOSED,
        Self::WINDOW_UNHIDDEN,
        Self::WINDOW_HIDDEN,
        // Self::WINDOW_CREATED,
        Self::WINDOW_DESTROYED,
    ];
}

// https://github.com/koekeishiya/yabai/blob/ff42ceadc92dfc50df63b73e3e1384b8b4059864/src/mission_control.c#L6
extern "C" fn register_conn_callback(
    event: CGSEvent,
    data: *mut c_void,
    _data_len: usize,
    _ctx: *mut c_void,
    _cid: i32,
) {
    let evt = match event {
        CGSEvent::WINDOW_CLOSED => {
            let details = unsafe { &*(data as *mut WinDetails) };
            if details.wid == ROOT.0 {
                return;
            }

            Event::WindowDestroyed {
                id: WinId(details.wid),
            }
        }

        CGSEvent::WINDOW_UNHIDDEN => {
            let details = unsafe { &*(data as *mut WinDetails) };
            if details.wid == ROOT.0 {
                return;
            }

            Event::WindowDeminiaturized {
                id: WinId(details.wid),
            }
        }

        CGSEvent::WINDOW_HIDDEN => {
            let details = unsafe { &*(data as *mut WinDetails) };
            if details.wid == ROOT.0 {
                return;
            }

            Event::WindowMiniaturized {
                id: WinId(details.wid),
            }
        }

        CGSEvent::WINDOW_DESTROYED => {
            let details = unsafe { &*(data as *mut WinDetails) };
            if details.wid == ROOT.0 {
                return;
            }

            Event::WindowDestroyed {
                id: WinId(details.wid),
            }
        }

        evt => {
            warn!(?evt, "received unknown CGS event");
            return;
        }
    };

    _ = EVENT_SENDER.wait().send(evt);
}

#[repr(C, packed(2))]
#[derive(Default, Debug)]
struct WinDetails {
    sid: u64,
    wid: u32,
}
