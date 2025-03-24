use crate::nsworkspace::{self as ns, INSNotification, INSNotificationCenter, INSWorkspace};
use accessibility::{attribute::AXAttribute, ui_element::AXUIElement};
use accessibility_sys::{
    AXError, AXIsProcessTrustedWithOptions, AXObserverAddNotification, AXObserverCreate,
    AXObserverGetRunLoopSource, AXObserverRef, AXObserverRemoveNotification,
    AXUIElementCreateSystemWide, AXUIElementRef, AXUIElementSetAttributeValue,
    AXUIElementSetMessagingTimeout, AXValueCreate, kAXErrorSuccess,
    kAXFocusedWindowChangedNotification, kAXPositionAttribute, kAXSizeAttribute,
    kAXTrustedCheckOptionPrompt, kAXValueTypeCGPoint, kAXValueTypeCGSize,
    kAXWindowCreatedNotification,
};
use block::ConcreteBlock;
use core_foundation::{
    base::{TCFType, ToVoid},
    dictionary::CFDictionary,
    runloop::{CFRunLoopAddSource, CFRunLoopGetCurrent, kCFRunLoopDefaultMode},
    string::CFString,
};
use core_foundation_sys::{
    base::CFRelease,
    dictionary::{
        CFDictionaryCreate, CFDictionaryRef, kCFTypeDictionaryKeyCallBacks,
        kCFTypeDictionaryValueCallBacks,
    },
    number::{CFNumberGetValue, CFNumberRef, kCFBooleanTrue, kCFNumberSInt32Type},
    string::CFStringRef,
};
use core_graphics::display::{self, CGDisplay, CGPoint, CGRect, CGSize, CGWindowID};
use std::ffi::c_void;

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

pub fn set_ax_timeout() {
    unsafe { AXUIElementSetMessagingTimeout(AXUIElementCreateSystemWide(), 1.0) };
}

// /Library/Developer/CommandLineTools/SDKs/MacOSX14.4.sdk/System/Library/Frameworks/AppKit.framework/Versions/C/Headers

// Private API that makes everything possible for mapping between the Accessibility API and
// CoreGraphics
unsafe extern "C" {
    pub fn _AXUIElementGetWindow(element: AXUIElementRef, out: *mut CGWindowID) -> AXError;
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

fn on_notif(n: ns::NSNotification) {
    unsafe {
        let name = n.name();
        let user_info = n.userInfo();
        println!("{name:?} {user_info:?}");
    }
}

pub fn register_observers() {
    let queue = ns::NSOperationQueue(main_queue() as *mut _ as ns::id);
    let mut block = ConcreteBlock::new(on_notif);
    let ptr = &mut block as *mut _ as *mut std::os::raw::c_void;

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

macro_rules! set_attr {
    ($axwin:expr, $val:expr, $ty:expr, $name:expr) => {
        unsafe {
            let val = AXValueCreate($ty, &mut $val as *mut _ as *mut c_void);
            let err = AXUIElementSetAttributeValue(
                $axwin.as_concrete_TypeRef(),
                CFString::new($name).as_concrete_TypeRef(),
                val as _,
            );

            if err == kAXErrorSuccess {
                Ok(())
            } else {
                Err(err)
            }
        }
    };
}

// FIXME: I'm mixing up the aeorospace logic around Apps vs Windows here

// kCGWindowAlpha = 1;
// kCGWindowBounds =     {
//     Height = 1107;
//     Width = 1200;
//     X = "-1200";
//     Y = 25;
// };
// kCGWindowIsOnscreen = 1;
// kCGWindowLayer = 0;
// kCGWindowMemoryUsage = 2176;
// kCGWindowNumber = 21247;
// kCGWindowOwnerName = Slack;
// kCGWindowOwnerPID = 83052;
// kCGWindowSharingState = 0;
// kCGWindowStoreType = 1;
#[derive(Debug, Clone)]
pub struct OsxWindow {
    pub win_id: u32,
    pub owner_pid: i32,
    pub window_layer: i32, // do we only care about layer 0?
    pub bounds: CGRect,
    pub owner: String,
    pub window_name: Option<String>,
    pub axwin: AXUIElement,
    pub observers: Vec<AXObserverWrapper>,
}

impl OsxWindow {
    pub fn set_size(&self, w: f64, h: f64) -> Result<(), AXError> {
        let mut s = CGSize::new(w, h);
        set_attr!(&self.axwin, s, kAXValueTypeCGSize, kAXSizeAttribute)
    }

    pub fn set_pos(&self, x: f64, y: f64) -> Result<(), AXError> {
        let mut p = CGPoint::new(x, y);
        set_attr!(&self.axwin, p, kAXValueTypeCGPoint, kAXPositionAttribute)
    }

    fn try_from_dict(dict: &CFDictionary) -> Option<Self> {
        fn get_string(dict: &CFDictionary, key: &str) -> Option<String> {
            dict.find(CFString::new(key).to_void()).map(|value| {
                unsafe { CFString::wrap_under_get_rule(*value as CFStringRef) }.to_string()
            })
        }

        fn get_i32(dict: &CFDictionary, key: &str) -> Option<i32> {
            let value = dict.find(CFString::new(key).to_void())?;
            let mut result = 0;
            unsafe {
                CFNumberGetValue(
                    *value as CFNumberRef,
                    kCFNumberSInt32Type,
                    (&mut result as *mut i32).cast(),
                )
            };

            Some(result)
        }

        fn get_dict(dict: &CFDictionary, key: &str) -> Option<CFDictionary> {
            let value = dict.find(CFString::new(key).to_void())?;
            Some(unsafe { CFDictionary::wrap_under_get_rule(*value as CFDictionaryRef) })
        }

        let win_id = get_i32(dict, "kCGWindowNumber")? as u32;
        let owner_pid = get_i32(dict, "kCGWindowOwnerPID")?;
        let window_layer = get_i32(dict, "kCGWindowLayer")?;
        let bounds = CGRect::from_dict_representation(&get_dict(dict, "kCGWindowBounds")?)?;
        let owner = get_string(dict, "kCGWindowOwnerName")?;
        let window_name = get_string(dict, "kCGWindowName");
        let axwin = get_axwindow(owner_pid, win_id).ok()?;

        let observers = vec![
            AXObserverWrapper::try_new(
                owner_pid,
                kAXWindowCreatedNotification,
                axwin.as_concrete_TypeRef(),
            )?,
            AXObserverWrapper::try_new(
                owner_pid,
                kAXFocusedWindowChangedNotification,
                axwin.as_concrete_TypeRef(),
            )?,
        ];

        Some(Self {
            win_id,
            owner_pid,
            window_layer,
            bounds,
            owner,
            window_name,
            axwin,
            observers,
        })
    }
}

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

unsafe extern "C" fn ax_observer_callback(
    _observer: AXObserverRef,
    _element: AXUIElementRef,
    notification: CFStringRef,
    _refcon: *mut c_void,
) {
    println!("{notification:?}");
}

impl AXObserverWrapper {
    pub fn try_new(pid: i32, notif: &str, ax: AXUIElementRef) -> Option<Self> {
        unsafe {
            let mut obs = std::ptr::null_mut();
            if AXObserverCreate(pid, ax_observer_callback, &mut obs as *mut _) != kAXErrorSuccess {
                return None;
            }
            let notif = CFString::new(notif);
            if AXObserverAddNotification(obs, ax, notif.as_concrete_TypeRef(), std::ptr::null_mut())
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

fn get_axwindow(pid: i32, winid: u32) -> Result<AXUIElement, &'static str> {
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

pub fn current_windows() -> Option<Vec<OsxWindow>> {
    let raw_infos = CGDisplay::window_list_info(
        display::kCGWindowListExcludeDesktopElements | display::kCGWindowListOptionOnScreenOnly,
        None,
    )?;
    let mut infos = Vec::new();

    for win_info in raw_infos.iter() {
        let dict = unsafe {
            CFDictionary::<*const c_void, *const c_void>::wrap_under_get_rule(
                *win_info as CFDictionaryRef,
            )
        };
        if let Some(info) = OsxWindow::try_from_dict(&dict) {
            infos.push(info);
        }
    }

    Some(infos)
}
