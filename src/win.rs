use crate::{
    nsworkspace::{
        INSRunningApplication,
        NSApplicationActivationOptions_NSApplicationActivateIgnoringOtherApps,
        NSRunningApplication, NSString_NSStringDeprecated,
    },
    sys::{APP_NOTIFICATIONS, AXObserverWrapper, WIN_NOTIFICATIONS, get_axwindow, rect_from_cg},
};
use accessibility::{
    AXAttribute, AXUIElementActions, AXUIElementAttributes, ui_element::AXUIElement,
};
use accessibility_sys::{
    AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementPerformAction,
    AXUIElementSetAttributeValue, AXValueCreate, kAXCloseButtonAttribute, kAXErrorSuccess,
    kAXPositionAttribute, kAXPressAction, kAXSizeAttribute, kAXValueTypeCGPoint,
    kAXValueTypeCGSize,
};
use core_foundation::{
    base::{TCFType, ToVoid},
    boolean::CFBoolean,
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
    window,
};
use penrose::{Result, WinId, custom_error, pure::geometry::Rect};
use std::ffi::{CStr, c_void};
use tracing::error;

pub type Pid = i32;

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
            window::kCGWindowListExcludeDesktopElements | window::kCGWindowListOptionOnScreenOnly,
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

#[derive(Debug, Clone)]
pub struct OsxApp {
    pub(crate) name: String,
    pub(crate) app: NSRunningApplication,
    // observers needs to be before axapp so we drop in the correct order
    pub(crate) _observers: Vec<AXObserverWrapper>,
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
                name,
                app,
                axapp: AXUIElement::wrap_under_get_rule(axapp),
                _observers: observers,
            })
        }
    }

    pub(crate) fn enhanced_user_interface_enabled(&self) -> bool {
        bool_attr(&self.axapp, "AXEnhancedUserInterface")
    }

    pub(crate) fn set_enhanced_user_interface(&self, on: bool) -> Result<()> {
        set_bool_attr(&self.axapp, "AXEnhancedUserInterface", on)
    }

    pub fn activate(&self) {
        unsafe {
            self.app.activateWithOptions_(
                NSApplicationActivationOptions_NSApplicationActivateIgnoringOtherApps,
            );
        }
    }

    pub(crate) fn focused_ax_window(&self) -> Result<AXUIElement> {
        self.axapp
            .attribute(&AXAttribute::focused_window())
            .map_err(|e| custom_error!("unable to get focused window for {}: {}", self.name, e))
    }
}
