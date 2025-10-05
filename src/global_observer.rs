//! Global observer for picking up notifications around application lifecycle events.
//!
//! [OsxApp][crate::app::OsxApp] and [OsxWindow][crate::win::OsxWindow] register their own
//! observers for notifications relating to individual apps / windows respectively. All events are
//! mapped into internal [Event]s for processing in the main window manager event loop.
use crate::{
    event::{EVENT_SENDER, Event},
    sys::current_screen_rects,
};
use objc2::{AnyThread, ClassType, MainThreadMarker, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationDidChangeScreenParametersNotification, NSRunningApplication,
    NSWorkspace, NSWorkspaceApplicationKey, NSWorkspaceDidActivateApplicationNotification,
    NSWorkspaceDidDeactivateApplicationNotification, NSWorkspaceDidHideApplicationNotification,
    NSWorkspaceDidLaunchApplicationNotification, NSWorkspaceDidTerminateApplicationNotification,
    NSWorkspaceDidUnhideApplicationNotification,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObject};
use std::mem;
use tracing::{error, trace, warn};

#[derive(Debug)]
pub struct GlobalObserver {
    _inner: Retained<GlobalObserverInner>,
    _mtm: MainThreadMarker,
}

impl GlobalObserver {
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self {
            _inner: GlobalObserverInner::new(mtm),
            _mtm: mtm,
        }
    }
}

define_class! {
    #[unsafe(super(NSObject))]
    #[derive(Debug)]
    struct GlobalObserverInner;

    impl GlobalObserverInner {
        #[unsafe(method(recvAppEvent:))]
        fn recv_app_event(&self, notif: &NSNotification) {
            trace!(?notif, "got app event");
            self.handle_app_event(notif);
        }

        #[unsafe(method(recvScreenChangedEvent:))]
        fn recv_screen_changed_event(&self, notif: &NSNotification) {
            trace!(?notif, "got screen change event");
            self.handle_screen_changed_event(notif);
        }
    }
}

impl GlobalObserverInner {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        unsafe {
            let inner: Retained<Self> = msg_send![Self::alloc(), init];

            // register observers
            let ws = &NSWorkspace::sharedWorkspace();
            let nc = &ws.notificationCenter();

            let app_notifications = &[
                NSWorkspaceDidLaunchApplicationNotification,
                NSWorkspaceDidActivateApplicationNotification,
                NSWorkspaceDidHideApplicationNotification,
                NSWorkspaceDidUnhideApplicationNotification,
                NSWorkspaceDidDeactivateApplicationNotification,
                NSWorkspaceDidTerminateApplicationNotification,
            ];

            for notif_name in app_notifications {
                nc.addObserver_selector_name_object(
                    &inner,
                    sel!(recvAppEvent:),
                    Some(notif_name),
                    Some(ws),
                );
            }

            // screen change notifications need to be listened for on the default notification
            // center
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &inner,
                sel!(recvScreenChangedEvent:),
                Some(NSApplicationDidChangeScreenParametersNotification),
                Some(&NSApplication::sharedApplication(mtm)),
            );

            inner
        }
    }

    fn handle_app_event(&self, notif: &NSNotification) {
        let app = match self.running_application(notif) {
            Some(app) => app,
            None => return,
        };

        unsafe {
            let pid = app.processIdentifier();
            let name = &*notif.name();
            // let desc = app.description(); <- contains an LSASN key that is the PSN but it comes
            // from an internally produced debug repr
            // let data = app.get_ivar::<&c_void>("_asn");
            // tracing::warn!(?data, "LOOK AT LSASN");

            let evt = if name == NSWorkspaceDidLaunchApplicationNotification {
                Event::AppLaunched { pid }
            } else if name == NSWorkspaceDidActivateApplicationNotification {
                Event::AppActivated { pid }
            } else if name == NSWorkspaceDidDeactivateApplicationNotification {
                Event::AppDeactivated { pid }
            } else if name == NSWorkspaceDidHideApplicationNotification {
                Event::AppHidden { pid }
            } else if name == NSWorkspaceDidUnhideApplicationNotification {
                Event::AppUnhidden { pid }
            } else if name == NSWorkspaceDidTerminateApplicationNotification {
                Event::AppTerminated { pid }
            } else {
                error!(%name, "got unknown app event");
                return;
            };

            _ = EVENT_SENDER.wait().send(evt);
        }
    }

    fn handle_screen_changed_event(&self, _notif: &NSNotification) {
        let screen_rects = current_screen_rects(
            MainThreadMarker::new().expect("parent GlobalObserver contains a MainThreadMarker"),
        );

        _ = EVENT_SENDER
            .wait()
            .send(Event::ScreensChanged { screen_rects });
    }

    fn running_application(
        &self,
        notif: &NSNotification,
    ) -> Option<Retained<NSRunningApplication>> {
        let user_info = match notif.userInfo() {
            Some(info) => info,
            None => {
                warn!(?notif, "received notification without user info");
                return None;
            }
        };
        let app = match unsafe { user_info.valueForKey(NSWorkspaceApplicationKey) } {
            Some(app) => app,
            None => {
                warn!(?notif, "received notification without app object");
                return None;
            }
        };

        debug_assert!(app.class() == NSRunningApplication::class());
        let app: Retained<NSRunningApplication> = unsafe { mem::transmute(app) };

        Some(app)
    }
}
