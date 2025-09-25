//! A handle to an OSX window.
use crate::{
    Pid,
    sys::{AXObserverWrapper, bool_attr},
};
use accessibility::{AXAttribute, AXUIElement, AXUIElementActions, AXUIElementAttributes};
use accessibility_sys::{
    AXError, AXUIElementCopyAttributeValue, AXUIElementPerformAction, AXUIElementRef,
    AXUIElementSetAttributeValue, AXValueCreate, kAXCloseButtonAttribute, kAXErrorSuccess,
    kAXMovedNotification, kAXPositionAttribute, kAXPressAction, kAXResizedNotification,
    kAXSizeAttribute, kAXUIElementDestroyedNotification, kAXValueTypeCGPoint, kAXValueTypeCGSize,
    kAXWindowDeminiaturizedNotification, kAXWindowMiniaturizedNotification,
};
use core_foundation::{
    base::{TCFType, ToVoid},
    dictionary::CFDictionary,
    string::CFString,
};
use core_foundation_sys::{
    dictionary::CFDictionaryRef,
    number::{CFNumberGetValue, CFNumberRef, kCFNumberSInt32Type},
    string::CFStringRef,
};
use core_graphics::{
    display::{CGDisplay, CGPoint, CGRect, CGSize},
    window::{CGWindowID, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly},
};
use penrose::{Result, WinId, custom_error, pure::geometry::Rect};
use std::ffi::c_void;
use tracing::error;

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
                Err(custom_error!("unable to set {} attr: {}", $name, err))
            }
        }
    };
}

pub(crate) static WIN_NOTIFICATIONS: [&str; 5] = [
    kAXUIElementDestroyedNotification,
    kAXWindowDeminiaturizedNotification,
    kAXWindowMiniaturizedNotification,
    kAXMovedNotification,
    kAXResizedNotification,
];

/// A handle to a running OSX window
#[derive(Debug, Clone)]
pub struct OsxWindow {
    pub(crate) win_id: WinId,
    pub(crate) owner_pid: Pid,
    pub(crate) window_layer: i32, // do we only care about layer 0?
    pub(crate) bounds: Rect,
    pub(crate) owner: String,
    pub(crate) window_name: Option<String>,
    // observers needs to be before axwin so we drop in the correct order
    pub(crate) _observers: Vec<AXObserverWrapper>,
    pub(crate) axwin: AXUIElement,
}

unsafe impl Send for OsxWindow {}
unsafe impl Sync for OsxWindow {}

impl OsxWindow {
    pub fn current_windows() -> Vec<Self> {
        let raw_infos = CGDisplay::window_list_info(
            kCGWindowListExcludeDesktopElements | kCGWindowListOptionOnScreenOnly,
            None,
        );
        let mut infos = Vec::new();
        if raw_infos.is_none() {
            return infos;
        }

        for win_info in raw_infos.unwrap().iter() {
            let dict = unsafe {
                CFDictionary::<*const c_void, *const c_void>::wrap_under_get_rule(
                    *win_info as CFDictionaryRef,
                )
            };
            match OsxWindow::try_from_dict(&dict) {
                Ok(info) => infos.push(info),
                Err(penrose::Error::Custom(s)) if s == "Window not found" => (),
                Err(e) => error!("unable to parse window dict {e} {dict:?}"),
            }
        }

        infos
    }

    pub fn set_size(&self, w: f64, h: f64) -> Result<()> {
        let mut s = CGSize::new(w, h);
        set_attr!(&self.axwin, s, kAXValueTypeCGSize, kAXSizeAttribute)
    }

    pub fn set_pos(&self, x: f64, y: f64) -> Result<()> {
        let mut p = CGPoint::new(x, y);
        set_attr!(&self.axwin, p, kAXValueTypeCGPoint, kAXPositionAttribute)
    }

    pub fn raise(&self) -> Result<()> {
        self.axwin
            .set_main(true)
            .map_err(|e| custom_error!("unable to set main attr for window: {}", e))?;
        self.axwin
            .raise()
            .map_err(|e| custom_error!("unable to raise window: {}", e))
    }

    pub fn close(&self) -> Result<()> {
        unsafe {
            let button = std::ptr::null_mut();
            AXUIElementCopyAttributeValue(
                self.axwin.as_concrete_TypeRef(),
                CFString::new(kAXCloseButtonAttribute).as_concrete_TypeRef(),
                button,
            );
            if button.is_null() {
                return Err(custom_error!("unable to get close button"));
            }
            AXUIElementPerformAction(
                button as _,
                CFString::new(kAXPressAction).as_concrete_TypeRef(),
            );
        }

        Ok(())
    }

    pub fn is_fullscreen(&self) -> bool {
        bool_attr(&self.axwin, "AXFullScreen")
    }

    fn try_from_dict(dict: &CFDictionary) -> Result<Self> {
        fn get_string(dict: &CFDictionary, key: &str) -> Result<String> {
            dict.find(CFString::new(key).to_void())
                .map(|value| {
                    unsafe { CFString::wrap_under_get_rule(*value as CFStringRef) }.to_string()
                })
                .ok_or_else(|| custom_error!("unable to read {} key as string", key))
        }

        fn get_i32(dict: &CFDictionary, key: &str) -> Result<i32> {
            let value = dict
                .find(CFString::new(key).to_void())
                .ok_or_else(|| custom_error!("unable to read {} key as i32", key))?;
            let mut result = 0;
            unsafe {
                CFNumberGetValue(
                    *value as CFNumberRef,
                    kCFNumberSInt32Type,
                    (&mut result as *mut i32).cast(),
                )
            };

            Ok(result)
        }

        fn get_dict(dict: &CFDictionary, key: &str) -> Result<CFDictionary> {
            let value = dict
                .find(CFString::new(key).to_void())
                .ok_or_else(|| custom_error!("unable to read {} key as dict", key))?;
            Ok(unsafe { CFDictionary::wrap_under_get_rule(*value as CFDictionaryRef) })
        }

        let win_id = get_i32(dict, "kCGWindowNumber")? as u32;
        let owner_pid = get_i32(dict, "kCGWindowOwnerPID")?;
        let axwin =
            get_axwindow(owner_pid, win_id).ok_or_else(|| custom_error!("Window not found"))?;
        let window_layer = get_i32(dict, "kCGWindowLayer")?;
        let bounds = CGRect::from_dict_representation(&get_dict(dict, "kCGWindowBounds")?)
            .ok_or_else(|| custom_error!("unable to parse CGRect from dict"))?;
        let owner = get_string(dict, "kCGWindowOwnerName")?;
        let window_name = get_string(dict, "kCGWindowName").ok();
        let axref = axwin.as_concrete_TypeRef();
        // disgusting
        let id_ptr: *mut c_void = std::ptr::without_provenance_mut(win_id as usize);
        let observers = WIN_NOTIFICATIONS
            .into_iter()
            .map(|s| AXObserverWrapper::try_new(owner_pid, s, axref, id_ptr))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            win_id: WinId::from(win_id),
            owner_pid,
            window_layer,
            bounds: rect_from_cg(bounds),
            owner,
            window_name,
            axwin,
            _observers: observers,
        })
    }
}

fn rect_from_cg(r: CGRect) -> Rect {
    Rect::new(
        r.origin.x as i32,
        r.origin.y as i32,
        r.size.width as u32,
        r.size.height as u32,
    )
}

// /Library/Developer/CommandLineTools/SDKs/MacOSX14.4.sdk/System/Library/Frameworks/AppKit.framework/Versions/C/Headers

// Private API that makes everything possible for mapping between the Accessibility API and
// CoreGraphics
unsafe extern "C" {
    pub fn _AXUIElementGetWindow(element: AXUIElementRef, out: *mut CGWindowID) -> AXError;
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
