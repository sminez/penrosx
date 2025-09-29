//! Connection events
use crate::{bindings::HotKey, sys::Pid};
use penrose::{WinId, core::conn::ConnEvent};
use std::{
    fmt,
    sync::{OnceLock, mpsc::Sender},
};

pub(crate) static EVENT_SENDER: OnceLock<Sender<Event>> = OnceLock::new();

/// An OSX event that can be processed by Penrose
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Event {
    // App level
    AppActivated { pid: Pid },
    AppDeactivated { pid: Pid },
    AppLaunched { pid: Pid },
    AppTerminated { pid: Pid },
    AppHidden { pid: Pid },
    AppUnhidden { pid: Pid },
    WindowCreated { pid: Pid },
    FocusedWindowChanged { pid: Pid },
    // Window level
    UiElementDestroyed { id: WinId },
    WindowMiniturized { id: WinId },
    WindowDeminiturized { id: WinId },
    WindowMoved { id: WinId },
    WindowResized { id: WinId },
    // Bindings
    KeyPress { k: HotKey },
}

impl ConnEvent for Event {
    fn requires_pointer_warp(&self) -> bool {
        false
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
            WindowMiniturized { .. } => write!(f, "WindowMiniturized"),
            WindowDeminiturized { .. } => write!(f, "WindowDeminiturized"),
            WindowMoved { .. } => write!(f, "WindowMoved"),
            WindowResized { .. } => write!(f, "WindowResized"),
            KeyPress { .. } => write!(f, "KeyPress"),
        }
    }
}
