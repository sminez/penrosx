//! Connection events
use crate::{bindings::HotKey, sys::Pid};
use penrose::{WinId, core::conn::ConnEvent, pure::geometry::Rect};
use std::{
    fmt,
    sync::{OnceLock, mpsc::Sender},
};

pub(crate) static EVENT_SENDER: OnceLock<Sender<Event>> = OnceLock::new();

/// An OSX event that can be processed by Penrose
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Event {
    /// The screen dimensions have changed
    ScreensChanged {
        /// The new screen dimensions
        screen_rects: Vec<Rect>,
    },

    /// The given application is now active
    AppActivated {
        /// The application pid
        pid: Pid,
    },
    /// The given application is no longer active
    AppDeactivated {
        /// The application pid
        pid: Pid,
    },
    /// The given application has been launched
    AppLaunched {
        /// The application pid
        pid: Pid,
    },
    /// The given application has been terminated
    AppTerminated {
        /// The application pid
        pid: Pid,
    },
    /// The given application was hidden
    AppHidden {
        /// The application pid
        pid: Pid,
    },
    /// The given application was unhidden
    AppUnhidden {
        /// The application pid
        pid: Pid,
    },
    /// A new window has been created for he given application
    WindowCreated {
        /// The application pid
        pid: Pid,
    },
    /// The focused window of the given application has changed
    FocusedWindowChanged {
        /// The application pid
        pid: Pid,
    },

    /// A UI element for the given window has been destroyed
    UiElementDestroyed {
        /// The window ID
        id: WinId,
    },
    /// The given window has been miniaturized
    WindowMiniaturized {
        /// The window ID
        id: WinId,
    },
    /// The given window has been deminiaturized
    WindowDeminiaturized {
        /// The window ID
        id: WinId,
    },
    /// The given window has moved
    WindowMoved {
        /// The window ID
        id: WinId,
    },
    /// The given window has been resized
    WindowResized {
        /// The window ID
        id: WinId,
    },
    /// The given window has been destroyed
    WindowDestroyed {
        /// The window ID
        id: WinId,
    },

    /// A user provided key binding has been run
    KeyPress {
        /// The hotkey that was triggered
        k: HotKey,
    },
}

impl ConnEvent for Event {
    fn requires_pointer_warp(&self) -> bool {
        matches!(self, Self::FocusedWindowChanged { .. })
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Event::*;
        match self {
            AppActivated { .. } => write!(f, "AppActivated"),
            AppDeactivated { .. } => write!(f, "AppDeactivated"),
            AppLaunched { .. } => write!(f, "AppLaunched"),
            AppTerminated { .. } => write!(f, "AppTerminated"),
            AppHidden { .. } => write!(f, "AppHidden"),
            AppUnhidden { .. } => write!(f, "AppUnhidden"),
            WindowCreated { .. } => write!(f, "WindowCreated"),
            FocusedWindowChanged { .. } => write!(f, "FocusedWindowChanged"),
            UiElementDestroyed { .. } => write!(f, "UiElementDestroyed"),
            WindowMiniaturized { .. } => write!(f, "WindowMiniturized"),
            WindowDeminiaturized { .. } => write!(f, "WindowDeminiturized"),
            WindowDestroyed { .. } => write!(f, "WindowDestroyed"),
            WindowMoved { .. } => write!(f, "WindowMoved"),
            WindowResized { .. } => write!(f, "WindowResized"),
            KeyPress { .. } => write!(f, "KeyPress"),
            ScreensChanged { .. } => write!(f, "ScreensChanged"),
        }
    }
}
