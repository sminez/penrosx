use crate::{
    event::Event,
    sys::ax::ui_element::{
        AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXUIElementCreateSystemWide,
        AXUIElementSetMessagingTimeout, kAXTrustedCheckOptionPrompt,
    },
};
use core_foundation::boolean::kCFBooleanTrue;
use objc2::{class, msg_send, rc::autoreleasepool, runtime::AnyObject};
use std::{
    process::exit,
    sync::{OnceLock, mpsc::Sender},
};
use tracing::{info, trace};

mod app;
pub(crate) mod ax;
mod global_observer;
mod win;

pub(crate) use app::OsxApp;
pub(crate) use global_observer::GlobalObserver;
pub(crate) use win::OsxWindow;

pub(crate) static EVENT_SENDER: OnceLock<Sender<Event>> = OnceLock::new();

/// Check to see if we have been granted access to the accessibility APIs and if not, prompt the
/// user to grant access before exiting.
pub fn check_ax_permissions_and_prompt() {
    // SAFETY: FFI function is called without arguments
    if unsafe { AXIsProcessTrusted() } {
        trace!("AXIsProcessTrusted=true");
        return;
    }

    info!("AXIsProcessTrusted=false: prompting for permissions");
    autoreleasepool(|_| {
        // SAFETY: Arguments being constructed for AXIsProcessTrustedWithOptions are valid.
        // See the extensive safety docs for the msg_send macro for more details here if this
        // ever needs updating.
        unsafe {
            let dict: *mut AnyObject = msg_send![
                class!(NSDictionary),
                dictionaryWithObjects: [kCFBooleanTrue as *mut AnyObject].as_ptr(),
                forKeys: [kAXTrustedCheckOptionPrompt as *mut AnyObject].as_ptr(),
                count: 1usize
            ];

            AXIsProcessTrustedWithOptions(dict.cast());
        }
    });

    info!("Exiting to pick up permissions changes");
    exit(0);
}

/// Set the process wide AX API messaging timeout to 1s
pub fn set_ax_timeout() {
    // SAFETY: args to AXUIElementSetMessagingTimeout are valid
    unsafe { AXUIElementSetMessagingTimeout(AXUIElementCreateSystemWide(), 1.0) };
}
