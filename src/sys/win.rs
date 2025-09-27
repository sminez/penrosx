//! A handle to an OSX window.
use crate::{
    Pid,
    sys::{
        AXObserverWrapper,
        ax::{
            actions::AXUIElementActions,
            attribute::{AXAttribute, AXUIElementAttributes},
            error::AXError,
            notification::{
                AX_MOVED, AX_RESIZED, AX_UI_ELEMENT_DESTROYED, AX_WINDOW_DEMINIATURIZED,
                AX_WINDOW_MINIATURIZED,
            },
            ui_element::{AXUIElement, AXUIElementRef},
        },
        bool_attr,
    },
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

pub(crate) static WIN_NOTIFICATIONS: [&str; 5] = [
    AX_UI_ELEMENT_DESTROYED,
    AX_WINDOW_DEMINIATURIZED,
    AX_WINDOW_MINIATURIZED,
    AX_MOVED,
    AX_RESIZED,
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

    // Debug includes the details for all of the attached observers and the AX UI element
    pub fn string_details(&self) -> String {
        format!(
            "Window(id={}, pid={}, owner={}, name={:?}, layer={}, bounds={:?})",
            self.win_id,
            self.owner_pid,
            self.owner,
            self.window_name,
            self.window_layer,
            self.bounds
        )
    }

    pub fn set_size(&self, w: f64, h: f64) -> Result<()> {
        self.axwin.set_size(CGSize::new(w, h))
    }

    pub fn set_pos(&self, x: f64, y: f64) -> Result<()> {
        self.axwin.set_position(CGPoint::new(x, y))
    }

    pub fn raise(&self) -> Result<()> {
        self.axwin.set_main(true)?;
        self.axwin.raise()
    }

    pub fn close(&self) -> Result<()> {
        self.axwin.close_button()?.press()
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
            if _AXUIElementGetWindow(ax_window as AXUIElementRef, &mut id).is_success()
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
