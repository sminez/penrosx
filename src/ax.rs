use crate::nsworkspace::{
    self as ns, INSArray, INSNotification, INSNotificationCenter, INSRunningApplication,
    INSWorkspace, NSArray, NSRunningApplication, NSWorkspace,
    NSWorkspace_NSWorkspaceRunningApplications,
};
use accessibility::{attribute::AXAttribute, ui_element::AXUIElement};
use accessibility_sys::{
    AXError, AXIsProcessTrustedWithOptions, AXObserverAddNotification, AXObserverCreate,
    AXObserverGetRunLoopSource, AXObserverRef, AXObserverRemoveNotification,
    AXUIElementCreateSystemWide, AXUIElementRef, AXUIElementSetMessagingTimeout, kAXErrorSuccess,
    kAXTrustedCheckOptionPrompt,
};
use block::ConcreteBlock;
use core_foundation::{
    base::TCFType,
    runloop::{CFRunLoopAddSource, CFRunLoopGetCurrent, kCFRunLoopDefaultMode},
    string::CFString,
};
use core_foundation_sys::string::CFStringRef;
use core_foundation_sys::{
    base::CFRelease,
    dictionary::{
        CFDictionaryCreate, kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks,
    },
    number::kCFBooleanTrue,
};
use core_graphics::display::CGWindowID;
use std::ffi::c_void;

// /Library/Developer/CommandLineTools/SDKs/MacOSX14.4.sdk/System/Library/Frameworks/AppKit.framework/Versions/C/Headers

// Private API that makes everything possible for mapping between the Accessibility API and
// CoreGraphics
unsafe extern "C" {
    pub fn _AXUIElementGetWindow(element: AXUIElementRef, out: *mut CGWindowID) -> AXError;
}

/// Check whether or not the current process has access to the AX APIs
pub fn proc_is_ax_trusted() -> bool {
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt as *const _];
        let values = [kCFBooleanTrue as *const _];
        let kc = &kCFTypeDictionaryKeyCallBacks;
        let kv = &kCFTypeDictionaryValueCallBacks;

        let dict = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            kc as *const _,
            kv as *const _,
        );

        let res = AXIsProcessTrustedWithOptions(dict);
        CFRelease(dict.cast());

        res
    }
}

/// Set the process wide AX API messaging timeout to 1s
pub fn set_ax_timeout() {
    unsafe { AXUIElementSetMessagingTimeout(AXUIElementCreateSystemWide(), 1.0) };
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct dispatch_object_s {
    _private: [u8; 0],
}
#[allow(non_camel_case_types)]
pub type dispatch_queue_t = *mut dispatch_object_s;
#[allow(non_camel_case_types)]
pub type dispatch_object_t = *mut dispatch_object_s;

#[cfg_attr(target_os = "macos", link(name = "System", kind = "dylib"))]
unsafe extern "C" {
    static _dispatch_main_q: dispatch_object_s;
    fn dispatch_retain(object: dispatch_object_t);
}

fn main_queue() -> dispatch_queue_t {
    unsafe {
        let q = &_dispatch_main_q as *const _ as dispatch_queue_t;
        dispatch_retain(q);
        q
    }
}

// TODO: this needs to push the notifications into a channel for processing
fn on_notif(n: ns::NSNotification) {
    unsafe {
        let name = n.name();
        let user_info = n.userInfo();
        println!("{name:?} {user_info:?}");
    }
}

/// Register NSWorkspace observers for application notifications
pub fn register_observers() {
    let queue = ns::NSOperationQueue(main_queue() as *mut _ as ns::id);
    let mut block = ConcreteBlock::new(on_notif);
    let ptr = &mut block as *mut _ as *mut c_void;

    unsafe {
        let nc = ns::NSWorkspace::sharedWorkspace().notificationCenter();
        let names = [
            ns::NSWorkspaceDidLaunchApplicationNotification,
            ns::NSWorkspaceDidActivateApplicationNotification,
            // ns::NSWorkspaceDidHideApplicationNotification,
            ns::NSWorkspaceDidUnhideApplicationNotification,
            ns::NSWorkspaceDidDeactivateApplicationNotification,
            ns::NSWorkspaceDidTerminateApplicationNotification,
        ];

        for name in names {
            nc.addObserverForName_object_queue_usingBlock_(name, std::ptr::null_mut(), queue, ptr);
        }
    }
}

pub(crate) fn running_applications() -> Vec<NSRunningApplication> {
    unsafe {
        let arr = NSWorkspace::sharedWorkspace().runningApplications();
        let count = <NSArray as INSArray<NSRunningApplication>>::count(&arr);
        let mut apps = Vec::with_capacity(count as usize);

        for i in 0..count {
            let app = NSRunningApplication(
                <NSArray as INSArray<NSRunningApplication>>::objectAtIndex_(&arr, i),
            );
            if app.activationPolicy() == 0 {
                apps.push(app);
            }
        }

        apps
    }
}

/// Attempt to get an [AXUIElement] for the accessibility API for the given application window
/// (identified by pid and window id)
pub(crate) fn get_axwindow(pid: i32, winid: u32) -> Result<AXUIElement, &'static str> {
    let attr = AXUIElement::application(pid)
        .attribute(&AXAttribute::windows())
        .map_err(|_| "Failed to get windows attr")?;

    for ax_window in attr.get_all_values().into_iter() {
        unsafe {
            let mut id: CGWindowID = 0;
            if _AXUIElementGetWindow(ax_window as AXUIElementRef, &mut id) == kAXErrorSuccess
                && id == winid
            {
                return Ok(AXUIElement::wrap_under_get_rule(
                    ax_window as AXUIElementRef,
                ));
            }
        }
    }

    Err("Window not found")
}

/// Drop handle around an AXObserverRef
#[derive(Debug, Clone)]
pub struct AXObserverWrapper {
    obs: AXObserverRef,
    ax: AXUIElementRef,
    notif: CFString,
}

impl Drop for AXObserverWrapper {
    fn drop(&mut self) {
        unsafe {
            AXObserverRemoveNotification(self.obs, self.ax, self.notif.as_concrete_TypeRef());
        }
    }
}

// TODO: this needs to push details into a channel for processing
unsafe extern "C" fn ax_observer_callback(
    _observer: AXObserverRef,
    _element: AXUIElementRef,
    notification: CFStringRef,
    _refcon: *mut c_void,
) {
    println!("{notification:?}");
}

impl AXObserverWrapper {
    pub fn try_new(pid: i32, notif: &str, ax: AXUIElementRef, data: *mut c_void) -> Option<Self> {
        unsafe {
            let mut obs = std::ptr::null_mut();
            if AXObserverCreate(pid, ax_observer_callback, &mut obs as *mut _) != kAXErrorSuccess {
                return None;
            }
            let notif = CFString::new(notif);
            if AXObserverAddNotification(obs, ax, notif.as_concrete_TypeRef(), data)
                == kAXErrorSuccess
            {
                CFRunLoopAddSource(
                    CFRunLoopGetCurrent(),
                    AXObserverGetRunLoopSource(obs),
                    kCFRunLoopDefaultMode,
                );

                return Some(Self { obs, ax, notif });
            }

            None
        }
    }
}
