use crate::{event::Event, sys::EVENT_SENDER};
use accessibility_sys::{
    AXObserverAddNotification, AXObserverCreate, AXObserverGetRunLoopSource, AXObserverRef,
    AXObserverRemoveNotification, AXUIElementRef, kAXErrorSuccess,
    kAXFocusedWindowChangedNotification, kAXMovedNotification, kAXResizedNotification,
    kAXUIElementDestroyedNotification, kAXWindowCreatedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification,
};
use core_foundation::{
    base::TCFType,
    runloop::{CFRunLoopAddSource, CFRunLoopGetMain, kCFRunLoopDefaultMode},
    string::{CFString, CFStringRef},
};
use core_foundation_sys::base::{CFRelease, CFRetain};
use penrose::{Result, custom_error};
use std::ffi::c_void;
use tracing::{error, trace};

/// A wrapper around an accessibility observer to give us an API we can work with
#[derive(Debug, Clone)]
pub struct AXObserverWrapper {
    obs: AXObserverRef,
    ax: AXUIElementRef,
    notif: CFString,
}

// SAFETY: we're only holding references to shared data
unsafe impl Send for AXObserverWrapper {}
// SAFETY: we're only holding references to shared data
unsafe impl Sync for AXObserverWrapper {}

impl Drop for AXObserverWrapper {
    fn drop(&mut self) {
        // SAFETY: we are the sole owner of the observer at this point
        unsafe {
            AXObserverRemoveNotification(self.obs, self.ax, self.notif.as_concrete_TypeRef());
            CFRelease(self.obs as *const _);
        }
    }
}

impl AXObserverWrapper {
    pub fn try_new(pid: i32, notif: &str, ax: AXUIElementRef, data: *mut c_void) -> Result<Self> {
        // SAFETY: pointers being used are valid
        unsafe {
            let mut obs = std::ptr::null_mut();
            let err = AXObserverCreate(pid, ax_observer_callback, &mut obs as *mut _);
            if err != kAXErrorSuccess {
                return Err(custom_error!("unable to create ax observer: {}", err));
            }
            CFRetain(obs as *const _);
            let notif = CFString::new(notif);
            let err = AXObserverAddNotification(obs, ax, notif.as_concrete_TypeRef(), data);
            if err != kAXErrorSuccess {
                return Err(custom_error!(
                    "unable to add notification to ax observer: {}",
                    err
                ));
            }

            CFRunLoopAddSource(
                CFRunLoopGetMain(),
                AXObserverGetRunLoopSource(obs),
                kCFRunLoopDefaultMode,
            );

            Ok(Self { obs, ax, notif })
        }
    }
}

unsafe extern "C" fn ax_observer_callback(
    _observer: AXObserverRef,
    _element: AXUIElementRef,
    notification: CFStringRef,
    p: *mut c_void,
) {
    // SAFETY: notification pointer is assumed to be valid
    let notif = unsafe { CFString::wrap_under_get_rule(notification) }.to_string();

    #[allow(non_upper_case_globals, reason = "accessibility_sys crate")]
    let evt = match notif.as_str() {
        kAXWindowCreatedNotification => Event::WindowCreated { pid: p.addr() as _ },
        kAXFocusedWindowChangedNotification => Event::FocusedWindowChanged { pid: p.addr() as _ },
        kAXUIElementDestroyedNotification => Event::UiElementDestroyed {
            id: (p.addr() as u32).into(),
        },
        kAXWindowDeminiaturizedNotification => Event::WindowDeminiturized {
            id: (p.addr() as u32).into(),
        },
        kAXWindowMiniaturizedNotification => Event::WindowMiniturized {
            id: (p.addr() as u32).into(),
        },
        kAXMovedNotification => Event::WindowMoved {
            id: (p.addr() as u32).into(),
        },
        kAXResizedNotification => Event::WindowResized {
            id: (p.addr() as u32).into(),
        },

        s => {
            error!("dropping unknown notification: {s}");
            return;
        }
    };

    if let Some(tx) = EVENT_SENDER.get() {
        trace!(?evt, "ax observer notification received");
        _ = tx.send(evt);
    }
}
