use crate::{
    ax::{AXObserverWrapper, get_axwindow},
    nsworkspace::{INSRunningApplication, NSRunningApplication, NSString_NSStringDeprecated},
};
use accessibility::ui_element::AXUIElement;
use accessibility_sys::{
    AXUIElementCreateApplication, AXUIElementSetAttributeValue, AXValueCreate, kAXErrorSuccess,
    kAXFocusedWindowChangedNotification, kAXMovedNotification, kAXPositionAttribute,
    kAXResizedNotification, kAXSizeAttribute, kAXUIElementDestroyedNotification,
    kAXValueTypeCGPoint, kAXValueTypeCGSize, kAXWindowCreatedNotification,
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
use core_graphics::display::{self, CGDisplay, CGPoint, CGRect, CGSize};
use penrose::{Result, custom_error};
use std::ffi::{CStr, c_void};

pub type Pid = i32;
pub type WinId = u32;

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

#[derive(Debug, Clone)]
pub struct OsxWindow {
    pub win_id: WinId,
    pub owner_pid: Pid,
    pub window_layer: i32, // do we only care about layer 0?
    pub bounds: CGRect,
    pub owner: String,
    pub window_name: Option<String>,
    // observers needs to be before axwin so we drop in the correct order
    pub observers: Vec<AXObserverWrapper>,
    pub axwin: AXUIElement,
}

impl OsxWindow {
    pub fn current_windows() -> Vec<Self> {
        let raw_infos = CGDisplay::window_list_info(
            display::kCGWindowListExcludeDesktopElements | display::kCGWindowListOptionOnScreenOnly,
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
                Ok(info) => {
                    println!("osxwindow created for {}", info.owner);
                    infos.push(info);
                }
                Err(penrose::Error::Custom(s)) if s == "Window not found" => (),
                Err(e) => println!("{e} {dict:?}"),
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
        let observers = [
            kAXUIElementDestroyedNotification,
            kAXWindowDeminiaturizedNotification,
            kAXWindowMiniaturizedNotification,
            kAXMovedNotification,
            kAXResizedNotification,
        ]
        .into_iter()
        .map(|s| AXObserverWrapper::try_new(owner_pid, s, axref, std::ptr::null_mut()))
        .collect::<Result<Vec<_>>>()?;

        Ok(Self {
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
pub struct OsxApp {
    pub pid: Pid,
    pub name: String,
    pub app: NSRunningApplication,
    // observers needs to be before axapp so we drop in the correct order
    pub observers: Vec<AXObserverWrapper>,
    pub axapp: AXUIElement,
}

impl OsxApp {
    pub fn try_new(app: NSRunningApplication) -> Result<Self> {
        unsafe {
            let pid = app.processIdentifier();
            let name = CStr::from_ptr(app.localizedName().cString())
                .to_string_lossy()
                .to_string();
            let axapp = AXUIElementCreateApplication(pid);
            let observers = [
                kAXWindowCreatedNotification,
                kAXFocusedWindowChangedNotification,
            ]
            .into_iter()
            .map(|s| AXObserverWrapper::try_new(pid, s, axapp, std::ptr::null_mut()))
            .collect::<Result<Vec<_>>>()?;

            Ok(Self {
                pid,
                name,
                app,
                axapp: AXUIElement::wrap_under_get_rule(axapp),
                observers,
            })
        }
    }
}
