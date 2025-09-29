use crate::event::{EVENT_SENDER, Event};
use objc2::{AnyThread, ClassType, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::{
    NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
    NSWorkspaceDidActivateApplicationNotification, NSWorkspaceDidDeactivateApplicationNotification,
    NSWorkspaceDidHideApplicationNotification, NSWorkspaceDidLaunchApplicationNotification,
    NSWorkspaceDidTerminateApplicationNotification, NSWorkspaceDidUnhideApplicationNotification,
};
use objc2_foundation::{NSNotification, NSObject};
use std::mem;
use tracing::{error, trace, warn};

#[derive(Debug)]
pub struct GlobalObserver {
    _inner: Retained<GlobalObserverInner>,
}

impl GlobalObserver {
    pub fn new() -> Self {
        Self {
            _inner: GlobalObserverInner::new(),
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
    }
}

impl GlobalObserverInner {
    fn new() -> Retained<Self> {
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

    fn running_application(
        &self,
        notif: &NSNotification,
    ) -> Option<Retained<NSRunningApplication>> {
        let user_info = match unsafe { notif.userInfo() } {
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
