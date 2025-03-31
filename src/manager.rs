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
    state::{Config, State},
};
use cocoa::{
    appkit::{
        NSApp, NSApplication, NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
    },
    base::nil,
    foundation::NSAutoreleasePool,
};
use std::{
    sync::mpsc::{Receiver, channel},
    thread::spawn,
};

pub struct WindowManager {
    cfg: Config,
}

impl WindowManager {
    pub fn new(cfg: Config) -> Self {
        Self { cfg }
    }

    pub fn run(self) {
        if !proc_is_ax_trusted() {
            panic!("process is not trusted for the AX API");
        }

        set_ax_timeout();

        // Create the app itself
        let (_pool, app) = unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let app = NSApp();
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

            (pool, app)
        };

        let (tx, rx) = channel();
        EVENT_SENDER.set(tx).unwrap();

        spawn(move || {
            let state = State::try_new(self.cfg).unwrap();
            handle_events(state, rx);
        });

        std::thread::sleep(std::time::Duration::from_millis(100));
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
}

fn handle_events(state: State, rx: Receiver<Event>) {
    for evt in rx.into_iter() {
        println!("got event: {evt:?}");
        if let Event::AxObserverWin { win_id, .. } = evt {
            match state.windows.get(&win_id) {
                None => println!("ax notification for unknown window"),
                Some(win) => println!("window is for {}", win.owner),
            }
        } else if let Event::AxObserverApp { pid, .. } = evt {
            match state.apps.get(&pid) {
                None => println!("ax notification for unknown app"),
                Some(app) => println!("app is {}", app.name),
            }
        }
    }
}
