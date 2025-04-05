use crate::{
    nsworkspace::{
        self as ns, CFRetain, INSArray, INSDictionary, INSNotification, INSNotificationCenter,
        INSRunningApplication, INSWorkspace, NSArray, NSDictionary, NSNotification,
        NSRunningApplication, NSWorkspace, NSWorkspace_NSWorkspaceRunningApplications, id,
    },
    win::Pid,
};
use accessibility::{attribute::AXAttribute, ui_element::AXUIElement};
use accessibility_sys::{
    AXError, AXIsProcessTrustedWithOptions, AXObserverAddNotification, AXObserverCreate,
    AXObserverGetRunLoopSource, AXObserverRef, AXObserverRemoveNotification,
    AXUIElementCreateSystemWide, AXUIElementRef, AXUIElementSetMessagingTimeout, kAXErrorSuccess,
    kAXFocusedWindowChangedNotification, kAXMovedNotification, kAXResizedNotification,
    kAXTrustedCheckOptionPrompt, kAXUIElementDestroyedNotification, kAXWindowCreatedNotification,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification,
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
use core_graphics::display::{CGRect, CGWindowID};
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Object, Sel},
    sel, sel_impl,
};
use penrose::{Result, WinId, custom_error, pure::geometry::Rect};
use std::{
    ffi::c_void,
    fmt,
    sync::{OnceLock, mpsc::Sender},
};
use tracing::error;

pub(crate) static EVENT_SENDER: OnceLock<Sender<Event>> = OnceLock::new();

pub(crate) const APP_NOTIFICATIONS: [&str; 2] = [
    kAXWindowCreatedNotification,
    kAXFocusedWindowChangedNotification,
];

pub(crate) const WIN_NOTIFICATIONS: [&str; 5] = [
    kAXUIElementDestroyedNotification,
    kAXWindowDeminiaturizedNotification,
    kAXWindowMiniaturizedNotification,
    kAXMovedNotification,
    kAXResizedNotification,
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Event {
    // App level
    AppActivated { pid: Pid },
    AppDeactivated { pid: Pid },
    AppLaunched { pid: Pid },
    AppTerminated { pid: Pid },
    AppHidden { pid: Pid },
    AppUnhidden { pid: Pid },
    WindowCreated { pid: Pid },
    FocusedWindowChanged { pid: Pid },
    // Window level
    UiElementDestroyed { id: WinId },
    WindowMiniturized { id: WinId },
    WindowDeminiturized { id: WinId },
    WindowMoved { id: WinId },
    WindowResized { id: WinId },
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Event::*;
        match self {
            AppActivated { .. } => write!(f, "AppActivated"),
            AppDeactivated { .. } => write!(f, "AppDeactivated"),
            AppLaunched { .. } => write!(f, "AppLaunched"),
            AppTerminated { .. } => write!(f, "AppTerminated"),
            AppHidden { .. } => write!(f, "AppHidden"),
            AppUnhidden { .. } => write!(f, "AppUnhidden"),
            WindowCreated { .. } => write!(f, "WindowCreated"),
            FocusedWindowChanged { .. } => write!(f, "FocusedWindowChanged"),
            UiElementDestroyed { .. } => write!(f, "UiElementDestroyed"),
            WindowMiniturized { .. } => write!(f, "WindowMiniturized"),
            WindowDeminiturized { .. } => write!(f, "WindowDeminiturized"),
            WindowMoved { .. } => write!(f, "WindowMoved"),
            WindowResized { .. } => write!(f, "WindowResized"),
        }
    }
}

macro_rules! impl_handlers {
    ($($fn:ident, $enum:ident;)+) => {
        $(extern "C" fn $fn(_this: &mut Object, _cmd: Sel, id: id) {
            unsafe {
                let pid = pid_from_user_info(NSNotification(id).userInfo());
                _ = EVENT_SENDER.get().unwrap().send(Event::$enum { pid });
            }
        })+

        pub fn global_observer() -> id {
            let sup = class!(NSObject);
            let mut decl = ClassDecl::new("GlobalObserver", sup).unwrap();
            unsafe {
                $(decl.add_method(sel!($fn:), $fn as extern "C" fn(&mut Object, Sel, id));)+
                let cls = decl.register();

                msg_send![cls, new]
            }
        }
    };
}

fn pid_from_user_info(d: NSDictionary) -> i32 {
    unsafe {
        let obj = CFString::new("NSWorkspaceApplicationKey");
        let k = obj.as_CFTypeRef() as *mut _;
        let app = NSRunningApplication(
            <NSDictionary as INSDictionary<ns::NSString, Object>>::objectForKey_(&d, k),
        );
        app.processIdentifier()
    }
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
    p: *mut c_void,
) {
    let s = unsafe { CFString::wrap_under_get_rule(notification) }.to_string();

    let evt = match s.as_str() {
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
                sel!(app_launched:),
            ),
            (
                ns::NSWorkspaceDidActivateApplicationNotification,
                sel!(app_activated:),
            ),
            (
                ns::NSWorkspaceDidHideApplicationNotification,
                sel!(app_hidden:),
            ),
            (
                ns::NSWorkspaceDidUnhideApplicationNotification,
                sel!(app_unhidden:),
            ),
            (
                ns::NSWorkspaceDidDeactivateApplicationNotification,
                sel!(app_deactivated:),
            ),
            (
                ns::NSWorkspaceDidTerminateApplicationNotification,
                sel!(app_terminated:),
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

unsafe impl Send for AXObserverWrapper {}
unsafe impl Sync for AXObserverWrapper {}

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

pub(crate) fn rect_from_cg(r: CGRect) -> Rect {
    Rect::new(
        r.origin.x as i32,
        r.origin.y as i32,
        r.size.width as u32,
        r.size.height as u32,
    )
}
