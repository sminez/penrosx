use crate::event::Event;
use accessibility::{AXAttribute, AXUIElement};
use accessibility_sys::{
    AXUIElementCreateSystemWide, AXUIElementSetAttributeValue, AXUIElementSetMessagingTimeout,
    kAXErrorSuccess,
};
use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};
use objc2::{class, msg_send, rc::autoreleasepool, runtime::AnyObject};
use penrose::{Result, custom_error};
use std::{
    ffi::c_void,
    process::exit,
    sync::{OnceLock, mpsc::Sender},
};
use tracing::{info, trace};

mod app;
mod global_observer;
mod observer;
mod win;

pub(crate) use app::OsxApp;
pub(crate) use global_observer::GlobalObserver;
pub(crate) use observer::AXObserverWrapper;
pub(crate) use win::OsxWindow;

pub(crate) static EVENT_SENDER: OnceLock<Sender<Event>> = OnceLock::new();

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;

    static kAXTrustedCheckOptionPrompt: *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: *const c_void;
}

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

fn bool_attr(elem: &AXUIElement, attr: &str) -> bool {
    match elem.attribute(&AXAttribute::new(&CFString::new(attr))) {
        Ok(attr) => attr.downcast::<CFBoolean>() == Some(CFBoolean::true_value()),
        Err(_) => false,
    }
}

fn set_bool_attr(elem: &AXUIElement, attr: &str, val: bool) -> Result<()> {
    let val = if val {
        CFBoolean::true_value()
    } else {
        CFBoolean::false_value()
    };

    unsafe {
        let err = AXUIElementSetAttributeValue(
            elem.as_concrete_TypeRef(),
            CFString::new("AXEnhancedUserInterface").as_concrete_TypeRef(),
            val.as_concrete_TypeRef() as _,
        );

        if err == kAXErrorSuccess {
            Ok(())
        } else {
            Err(custom_error!("unable to set {} attr: {}", attr, err))
        }
    }
}
