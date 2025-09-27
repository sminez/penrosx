//! A handle to an OSX window.
use crate::{
    Pid,
    event::Event,
    sys::{
        EVENT_SENDER,
        ax::{
            actions::AXUIElementActions,
            attribute::AXUIElementAttributes,
            notification::{
                AX_MOVED, AX_RESIZED, AX_UI_ELEMENT_DESTROYED, AX_WINDOW_DEMINIATURIZED,
                AX_WINDOW_MINIATURIZED,
            },
            observer::Observer,
            ui_element::{AXUIElement, try_get_window_id},
        },
    },
};
use core_foundation::{
    base::{CFType, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use core_graphics::{
    display::{CGDisplay, CGPoint, CGRect, CGSize},
    window::{kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly},
};
use penrose::{Result, WinId, custom_error, pure::geometry::Rect};
use tracing::{error, trace};

pub(crate) static WIN_NOTIFICATIONS: [&str; 5] = [
    AX_UI_ELEMENT_DESTROYED,
    AX_WINDOW_DEMINIATURIZED,
    AX_WINDOW_MINIATURIZED,
    AX_MOVED,
    AX_RESIZED,
];

/// A handle to a running OSX window
#[derive(Debug)]
pub struct OsxWindow {
    pub(crate) win_id: WinId,
    pub(crate) owner_pid: Pid,
    pub(crate) window_layer: i32, // we only care about layer 0
    pub(crate) bounds: Rect,
    pub(crate) owner: String,
    pub(crate) window_name: Option<String>,
    // observers needs to be before axwin so we drop in the correct order
    pub(crate) _observer: Observer,
    pub(crate) axwin: AXUIElement,
}

unsafe impl Send for OsxWindow {}
unsafe impl Sync for OsxWindow {}

impl OsxWindow {
    pub fn current_windows() -> Vec<Self> {
        let raw_wins = CGDisplay::window_list_info(
            kCGWindowListExcludeDesktopElements | kCGWindowListOptionOnScreenOnly,
            None,
        );
        let mut wins = Vec::new();
        if raw_wins.is_none() {
            return wins;
        }

        for d in raw_wins.unwrap().iter() {
            let dict = unsafe { CFDictionary::<CFString, CFType>::wrap_under_get_rule(*d as _) };
            match OsxWindow::try_from_dict(&dict) {
                Ok(win) => wins.push(win),
                Err(penrose::Error::Custom(s)) if s == "Window not found" => (),
                Err(e) => error!("unable to parse window dict {e} {dict:?}"),
            }
        }

        wins
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
        self.axwin
            .fullscreen()
            .map(|val| val.into())
            .unwrap_or(false)
    }

    fn try_from_dict(dict: &CFDictionary<CFString, CFType>) -> Result<Self> {
        let win_id = try_get_i32(dict, "kCGWindowNumber")? as u32;
        let owner_pid = try_get_i32(dict, "kCGWindowOwnerPID")?;
        let axwin = get_axwindow(owner_pid, win_id).ok_or(custom_error!("Window not found"))?;
        let window_layer = try_get_i32(dict, "kCGWindowLayer")?;
        let bounds = try_get_bounds(dict)?;
        let owner = try_get_string(dict, "kCGWindowOwnerName")?;
        let window_name = try_get_string(dict, "kCGWindowName").ok();

        let id = WinId(win_id);
        let observer = Observer::try_new(owner_pid, move |notif| {
            let evt = match notif {
                AX_UI_ELEMENT_DESTROYED => Event::UiElementDestroyed { id },
                AX_WINDOW_DEMINIATURIZED => Event::WindowDeminiturized { id },
                AX_WINDOW_MINIATURIZED => Event::WindowMiniturized { id },
                AX_MOVED => Event::WindowMoved { id },
                AX_RESIZED => Event::WindowResized { id },

                s => {
                    error!("dropping unknown window notification: {s}");
                    return;
                }
            };

            trace!(?evt, "ax observer notification received");
            _ = EVENT_SENDER.wait().send(evt);
        })?;

        for notif in WIN_NOTIFICATIONS.iter() {
            observer.add_notification(&axwin, notif)?
        }

        Ok(Self {
            win_id: WinId::from(win_id),
            owner_pid,
            window_layer,
            bounds,
            owner,
            window_name,
            axwin,
            _observer: observer,
        })
    }
}

fn try_get_string(dict: &CFDictionary<CFString, CFType>, key: &str) -> Result<String> {
    dict.find(CFString::new(key))
        .and_then(|value| value.downcast())
        .map(|s: CFString| s.to_string())
        .ok_or_else(|| custom_error!("unable to read {} key as string", key))
}

fn try_get_i32(dict: &CFDictionary<CFString, CFType>, key: &str) -> Result<i32> {
    dict.find(CFString::new(key))
        .and_then(|value| value.downcast())
        .and_then(|n: CFNumber| n.to_i32())
        .ok_or_else(|| custom_error!("unable to read {} key as i32", key))
}

fn try_get_bounds(dict: &CFDictionary<CFString, CFType>) -> Result<Rect> {
    let r = CGRect::from_dict_representation(
        &dict
            .find(CFString::new("kCGWindowBounds"))
            .and_then(|value| value.downcast())
            .ok_or_else(|| custom_error!("unable to read kCGWindowBounds key as dict"))?,
    )
    .ok_or_else(|| custom_error!("unable to parse CGRect from dict"))?;

    Ok(Rect::new(
        r.origin.x as i32,
        r.origin.y as i32,
        r.size.width as u32,
        r.size.height as u32,
    ))
}

/// Attempt to get an [AXUIElement] for the accessibility API for the given application window
/// (identified by pid and window id)
fn get_axwindow(pid: i32, winid: u32) -> Option<AXUIElement> {
    for elem in AXUIElement::application(pid).windows().ok()?.iter() {
        if let Ok(id) = try_get_window_id(elem.as_concrete_TypeRef())
            && id == winid
        {
            return Some(unsafe { AXUIElement::wrap_under_get_rule(elem.as_concrete_TypeRef()) });
        }
    }

    None
}
