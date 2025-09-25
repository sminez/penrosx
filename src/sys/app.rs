//! A handle to an OSX application.
use crate::sys::{AXObserverWrapper, bool_attr, set_bool_attr};
use accessibility::{attribute::AXAttribute, ui_element::AXUIElement};
use accessibility_sys::{
    AXUIElementCreateApplication, kAXFocusedWindowChangedNotification, kAXWindowCreatedNotification,
};
use core_foundation::base::TCFType;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplicationActivationOptions, NSApplicationActivationPolicy, NSRunningApplication,
    NSWorkspace,
};
use penrose::{Result, custom_error};
use std::ffi::c_void;

static APP_NOTIFICATIONS: [&str; 2] = [
    kAXWindowCreatedNotification,
    kAXFocusedWindowChangedNotification,
];

#[derive(Debug)]
pub struct OsxApp {
    pub(crate) name: String,
    pub(crate) app: Retained<NSRunningApplication>,
    // observers needs to be before axapp so we drop in the correct order
    pub(crate) _observers: Vec<AXObserverWrapper>,
    pub(crate) axapp: AXUIElement,
}

unsafe impl Send for OsxApp {}
unsafe impl Sync for OsxApp {}

impl OsxApp {
    pub(crate) fn running_applications() -> Vec<Retained<NSRunningApplication>> {
        unsafe {
            NSWorkspace::sharedWorkspace()
                .runningApplications()
                .into_iter()
                .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
                .collect()
        }
    }

    pub fn try_new(app: Retained<NSRunningApplication>) -> Result<Self> {
        unsafe {
            let pid = app.processIdentifier();
            let name = app.localizedName().unwrap_or_default().to_string();
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

    // Debug includes the details for all of the attached observers and the AX UI element
    pub fn string_details(&self) -> String {
        format!("App(name={})", self.name)
    }

    pub(crate) fn enhanced_user_interface_enabled(&self) -> bool {
        bool_attr(&self.axapp, "AXEnhancedUserInterface")
    }

    pub(crate) fn set_enhanced_user_interface(&self, on: bool) -> Result<()> {
        set_bool_attr(&self.axapp, "AXEnhancedUserInterface", on)
    }

    pub fn activate(&self) {
        unsafe {
            self.app
                .activateWithOptions(NSApplicationActivationOptions::empty());
        }
    }

    pub(crate) fn focused_ax_window(&self) -> Result<AXUIElement> {
        self.axapp
            .attribute(&AXAttribute::focused_window())
            .map_err(|e| custom_error!("unable to get focused window for {}: {}", self.name, e))
    }
}
