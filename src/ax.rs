use crate::{
    nsworkspace::{
        self as ns, CFRetain, INSArray, INSNotificationCenter, INSRunningApplication, INSWorkspace,
        NSArray, NSRunningApplication, NSWorkspace, NSWorkspace_NSWorkspaceRunningApplications, id,
    },
    win::{APP_NOTIFICATIONS, Pid, WIN_NOTIFICATIONS, WinId},
};
use accessibility::{attribute::AXAttribute, ui_element::AXUIElement};
use accessibility_sys::{
    AXError, AXIsProcessTrustedWithOptions, AXObserverAddNotification, AXObserverCreate,
    AXObserverGetRunLoopSource, AXObserverRef, AXObserverRemoveNotification,
    AXUIElementCreateSystemWide, AXUIElementRef, AXUIElementSetMessagingTimeout, kAXErrorSuccess,
    kAXTrustedCheckOptionPrompt,
};
use core_foundation::{
    base::TCFType,
    runloop::{CFRunLoopAddSource, CFRunLoopGetMain, kCFRunLoopDefaultMode},
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
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Object, Sel},
    sel, sel_impl,
};
use penrose::{Result, custom_error};
use std::{
    ffi::c_void,
    sync::{OnceLock, mpsc::Sender},
};

pub(crate) static EVENT_SENDER: OnceLock<Sender<Event>> = OnceLock::new();

#[derive(Debug)]
pub(crate) enum Event {
    // App level
    AppActivated,
    AppDeactivated,
    AppLaunched,
    AppTerminated,
    AppHidden,
    AppUnhidden,
    // Window level
    AxObserverWin { notification: String, win_id: WinId },
    AxObserverApp { notification: String, pid: Pid },
}

macro_rules! impl_handlers {
    ($($fn:ident, $enum:ident;)+) => {
        $(extern "C" fn $fn(_this: &Object, _cmd: Sel) {
            _ = EVENT_SENDER.get().unwrap().send(Event::$enum);
        })+

        pub fn global_observer() -> id {
            let sup = class!(NSObject);
            let mut decl = ClassDecl::new("GlobalObserver", sup).unwrap();
            unsafe {
                $(decl.add_method(sel!($fn), $fn as extern "C" fn(&Object, Sel));)+
                let cls = decl.register();

                msg_send![cls, new]
            }
        }
    };
}

impl_handlers!(
    app_activated, AppActivated;
    app_deactivated, AppDeactivated;
    app_launched, AppLaunched;
    app_terminated, AppTerminated;
    app_hidden, AppHidden;
    app_unhidden, AppUnhidden;
);

unsafe extern "C" fn ax_observer_callback(
    _observer: AXObserverRef,
    _element: AXUIElementRef,
    notification: CFStringRef,
    refcon: *mut c_void,
) {
    let notification = unsafe { CFString::wrap_under_get_rule(notification) }.to_string();
    let evt = if WIN_NOTIFICATIONS.contains(&notification.as_str()) {
        Event::AxObserverWin {
            notification,
            win_id: refcon.addr() as WinId,
        }
    } else if APP_NOTIFICATIONS.contains(&notification.as_str()) {
        Event::AxObserverApp {
            notification,
            pid: refcon.addr() as Pid,
        }
    } else {
        println!("dropping unknown notification: {notification}");
        return;
    };

    if let Some(tx) = EVENT_SENDER.get() {
        _ = tx.send(evt);
    }
}

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

/// Register NSWorkspace observers for application notifications
pub fn register_observers(observer: id) {
    unsafe {
        let nc = ns::NSWorkspace::sharedWorkspace().notificationCenter();
        let handlers = [
            (
                ns::NSWorkspaceDidLaunchApplicationNotification,
                sel!(app_launched),
            ),
            (
                ns::NSWorkspaceDidActivateApplicationNotification,
                sel!(app_activated),
            ),
            (
                ns::NSWorkspaceDidHideApplicationNotification,
                sel!(app_hidden),
            ),
            (
                ns::NSWorkspaceDidUnhideApplicationNotification,
                sel!(app_unhidden),
            ),
            (
                ns::NSWorkspaceDidDeactivateApplicationNotification,
                sel!(app_deactivated),
            ),
            (
                ns::NSWorkspaceDidTerminateApplicationNotification,
                sel!(app_terminated),
            ),
        ];

        for (name, selector) in handlers {
            nc.addObserver_selector_name_object_(observer, selector, name, std::ptr::null_mut());
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
pub(crate) fn get_axwindow(pid: i32, winid: u32) -> Option<AXUIElement> {
    let attr = AXUIElement::application(pid)
        .attribute(&AXAttribute::windows())
        .ok()?;

    for ax_window in attr.get_all_values().into_iter() {
        unsafe {
            let mut id: CGWindowID = 0;
            if _AXUIElementGetWindow(ax_window as AXUIElementRef, &mut id) == kAXErrorSuccess
                && id == winid
            {
                return Some(AXUIElement::wrap_under_get_rule(
                    ax_window as AXUIElementRef,
                ));
            }
        }
    }

    None
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
            CFRelease(self.obs as *const _);
        }
    }
}

impl AXObserverWrapper {
    pub fn try_new(pid: i32, notif: &str, ax: AXUIElementRef, data: *mut c_void) -> Result<Self> {
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
