//! Window manager logic
use crate::{
    ax::{
        EVENT_SENDER, Event, global_observer, proc_is_ax_trusted, register_observers,
        set_ax_timeout,
    },
    nsworkspace::{
        INSRunningApplication,
        NSApplicationActivationOptions_NSApplicationActivateIgnoringOtherApps,
        NSRunningApplication,
    },
    state::State,
    win::{Pid, WinId},
};
use cocoa::{
    appkit::{
        NSApp, NSApplication, NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
    },
    base::nil,
    foundation::NSAutoreleasePool,
};
use std::{
    process::exit,
    sync::mpsc::{Receiver, channel},
    thread::spawn,
};
use tracing::info;

pub struct WindowManager {
    state: State,
}

impl WindowManager {
    pub fn new(state: State) -> Self {
        Self { state }
    }

    pub fn run(self) {
        if !proc_is_ax_trusted() {
            panic!("process is not trusted for the AX API");
        }

        set_ax_timeout();

        let (_pool, app) = unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let app = NSApp();
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

            (pool, app)
        };

        let (tx, rx) = channel();
        EVENT_SENDER.set(tx).unwrap();

        spawn(move || {
            self.handle_events(rx);
        });

        let global_observer = global_observer();
        register_observers(global_observer);

        unsafe {
            let current_app = NSRunningApplication::currentApplication();
            current_app.activateWithOptions_(
                NSApplicationActivationOptions_NSApplicationActivateIgnoringOtherApps,
            );
        }

        unsafe { app.run() };
    }

    fn handle_events(mut self, rx: Receiver<Event>) {
        use Event::*;

        for evt in rx.into_iter() {
            info!("got event: {evt:?}");
            match evt {
                AppActivated | AppDeactivated => self.handle_app_focus(),
                AppLaunched | AppTerminated => self.handle_app_open_close(),
                AppHidden | AppUnhidden => self.handle_app_visibility(),
                WindowCreated { pid } => self.handle_new_window(pid),
                UiElementDestroyed { id } => self.handle_window_close(id),
                FocusedWindowChanged { pid } => self.handle_app_focused_window(pid),
                WindowMoved { id } | WindowResized { id } => self.handle_window_position(id),
                WindowMiniturized { id } | WindowDeminiturized { id } => {
                    self.handle_window_visibility(id)
                }
            }
        }

        // Ensure that we exit when _our_ event loop completes even if the OSX app loop is still
        // ongoing
        exit(0);
    }

    fn handle_app_focus(&mut self) {
        info!("app focus changed, refreshing");
        self.refresh();
    }

    fn handle_app_open_close(&mut self) {
        info!("running apps changed, refreshing");
        self.refresh();
    }

    fn handle_app_visibility(&mut self) {
        info!("app visibility changed, refreshing");
        self.refresh();
    }

    fn handle_new_window(&mut self, pid: Pid) {
        info!("new window created for {pid}");
        self.refresh();
    }

    fn handle_app_focused_window(&mut self, pid: Pid) {
        info!("focused window for app changed {pid}");
        self.refresh();
    }

    fn handle_window_close(&mut self, id: WinId) {
        info!("window closed {id}");
        self.refresh();
    }

    fn handle_window_position(&mut self, id: WinId) {
        info!("window position changed {id}");
        self.refresh();
    }

    fn handle_window_visibility(&mut self, id: WinId) {
        info!("window visibility changed {id}");
        self.refresh();
    }

    fn refresh(&mut self) {}
}
