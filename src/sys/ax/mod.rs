//! Minimal bindings to the accessibility APIs for OSX
//!
//! Adapted from <https://github.com/eiz/accessibility> which is not being actively maintained and
//! pulls in conflicting versions of crates that I need elsewhere.
//! The API defined here is an internal implementation detail and not suitable for general purpose
//! use.
use crate::sys::ax::ui_element::{
    AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXUIElementCreateSystemWide,
    AXUIElementSetMessagingTimeout, kAXTrustedCheckOptionPrompt,
};
use core_foundation::boolean::kCFBooleanTrue;
use objc2::{class, msg_send, rc::autoreleasepool, runtime::AnyObject};
use penrose::{Result, custom_error};
use std::{mem::MaybeUninit, process::exit};
use tracing::{info, trace};

pub mod actions;
pub mod attribute;
pub mod error;
pub mod notification;
pub mod observer;
pub mod ui_element;
pub mod value;

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

pub(crate) unsafe fn ax_call<F, V>(f: F) -> Result<V>
where
    F: Fn(*mut V) -> error::AXError,
{
    let mut result = MaybeUninit::uninit();
    let err = (f)(result.as_mut_ptr());

    if err.is_err() {
        return Err(custom_error!("AX error: {}", err));
    }

    Ok(unsafe { result.assume_init() })
}

pub(crate) unsafe fn ax_call_void<F>(f: F) -> Result<()>
where
    F: Fn() -> error::AXError,
{
    let err = (f)();
    if err.is_err() {
        return Err(custom_error!("AX error: {}", err));
    }

    Ok(())
}
