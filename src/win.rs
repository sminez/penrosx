use crate::{
    nsworkspace::{INSRunningApplication, NSRunningApplication, NSString_NSStringDeprecated},
    sys::{APP_NOTIFICATIONS, AXObserverWrapper, WIN_NOTIFICATIONS, get_axwindow, rect_from_cg},
};
use accessibility::ui_element::AXUIElement;
use accessibility_sys::{
    AXUIElementCreateApplication, AXUIElementSetAttributeValue, AXValueCreate, kAXErrorSuccess,
    kAXPositionAttribute, kAXSizeAttribute, kAXValueTypeCGPoint, kAXValueTypeCGSize,
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
use penrose::{Result, Xid, custom_error, pure::geometry::Rect};
use std::ffi::{CStr, c_void};
use tracing::error;

pub type Pid = i32;
pub type WinId = Xid;

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
    pub(crate) win_id: WinId,
    pub(crate) owner_pid: Pid,
    pub(crate) window_layer: i32, // do we only care about layer 0?
    pub(crate) bounds: Rect,
    pub(crate) owner: String,
    pub(crate) window_name: Option<String>,
    // observers needs to be before axwin so we drop in the correct order
    pub(crate) observers: Vec<AXObserverWrapper>,
    pub(crate) axwin: AXUIElement,
}

unsafe impl Send for OsxWindow {}
unsafe impl Sync for OsxWindow {}

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
            win_id: Xid::from(win_id),
            owner_pid,
            window_layer,
            bounds: rect_from_cg(bounds),
            owner,
            window_name,
            axwin,
            observers,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OsxApp {
    pub(crate) pid: Pid,
    pub(crate) name: String,
    pub(crate) app: NSRunningApplication,
    // observers needs to be before axapp so we drop in the correct order
    pub(crate) observers: Vec<AXObserverWrapper>,
    pub(crate) axapp: AXUIElement,
}

unsafe impl Send for OsxApp {}
unsafe impl Sync for OsxApp {}

impl OsxApp {
    pub fn try_new(app: NSRunningApplication) -> Result<Self> {
        unsafe {
            let pid = app.processIdentifier();
            let name = CStr::from_ptr(app.localizedName().cString())
                .to_string_lossy()
                .to_string();
            let axapp = AXUIElementCreateApplication(pid);
            // disgusting
            let pid_ptr: *mut c_void = std::ptr::without_provenance_mut(pid as usize);
            let observers = APP_NOTIFICATIONS
                .into_iter()
                .map(|s| AXObserverWrapper::try_new(pid, s, axapp, pid_ptr))
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
