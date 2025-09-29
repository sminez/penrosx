//! A handle to an OSX application.
use crate::{
    event::{EVENT_SENDER, Event},
    sys::ax::{
        attribute::{AXAttribute, AXUIElementAttributes},
        notification::{AX_FOCUSED_WINDOW_CHANGED, AX_WINDOW_CREATED},
        observer::Observer,
        ui_element::AXUIElement,
    },
};
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplicationActivationOptions, NSApplicationActivationPolicy, NSRunningApplication,
    NSWorkspace,
};
use penrose::{Result, custom_error};
use tracing::{error, trace};

static APP_NOTIFICATIONS: [&str; 2] = [AX_WINDOW_CREATED, AX_FOCUSED_WINDOW_CHANGED];

#[derive(Debug)]
pub struct OsxApp {
    pub(crate) name: String,
    pub(crate) app: Retained<NSRunningApplication>,
    // observers needs to be before axapp so we drop in the correct order
    pub(crate) _observer: Observer,
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
            let axapp = AXUIElement::application(pid);

            let observer = Observer::try_new(pid, move |notif| {
                let evt = match notif {
                    AX_WINDOW_CREATED => Event::WindowCreated { pid },
                    AX_FOCUSED_WINDOW_CHANGED => Event::FocusedWindowChanged { pid },

                    s => {
                        error!("dropping unknown app notification: {s}");
                        return;
                    }
                };

                trace!(?evt, "ax observer notification received");
                _ = EVENT_SENDER.wait().send(evt);
            })?;

            for notif in APP_NOTIFICATIONS.iter() {
                observer.add_notification(&axapp, notif)?
            }

            Ok(Self {
                name,
                app,
                axapp,
                _observer: observer,
            })
        }
    }

    // Debug includes the details for all of the attached observers and the AX UI element
    pub fn string_details(&self) -> String {
        format!("App(name={})", self.name)
    }

    pub(crate) fn enhanced_user_interface_enabled(&self) -> bool {
        self.axapp
            .enhanced_user_interface()
            .map(|val| val.into())
            .unwrap_or(false)
    }

    pub(crate) fn set_enhanced_user_interface(&self, on: bool) -> Result<()> {
        self.axapp.set_enhanced_user_interface(on)
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
