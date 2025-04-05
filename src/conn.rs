//! A Conn impl for OSX
use crate::{
    nsworkspace::{
        INSRunningApplication,
        NSApplicationActivationOptions_NSApplicationActivateIgnoringOtherApps,
        NSRunningApplication,
    },
    sys::{
        EVENT_SENDER, Event, global_observer, proc_is_ax_trusted, register_observers,
        running_applications, set_ax_timeout,
    },
    win::{OsxApp, OsxWindow, Pid},
};
use accessibility::{AXAttribute, AXUIElement};
use cocoa::{
    appkit::{
        NSApp, NSApplication, NSApplicationActivationPolicy::NSApplicationActivationPolicyRegular,
    },
    base::nil,
    foundation::NSAutoreleasePool,
};
use core_graphics::{
    display::{CGDisplay, CGPoint},
    event::CGEvent,
    event_source::{CGEventSource, CGEventSourceStateID},
};
use penrose::{
    Color, Error, Result, WinId,
    core::{
        Config, State, WindowManager,
        bindings::{KeyBindings, KeyCode, MouseBindings, MouseState},
        conn::{Conn, ConnEvent, ConnExt},
    },
    custom_error,
    pure::geometry::{Point, Rect},
};
use std::{
    collections::HashMap,
    sync::{
        Mutex,
        mpsc::{Receiver, channel},
    },
    thread::spawn,
};
use tracing::{debug, info};

const ROOT: WinId = WinId(0);

macro_rules! win {
    ($self:ident, $id:expr) => {
        match $self.windows.get(&$id) {
            Some(win) => Ok(win),
            None => {
                $self.update_known_apps_and_windows();
                $self.windows.get(&$id).ok_or(Error::UnknownClient($id))
            }
        }
    };
}

macro_rules! app {
    ($self:ident, $pid:expr) => {
        match $self.apps.get(&$pid) {
            Some(app) => Ok(app),
            None => {
                $self.update_known_apps_and_windows();
                $self
                    .apps
                    .get(&$pid)
                    .ok_or(custom_error!("unknown app pid {}", $pid))
            }
        }
    };
}

#[derive(Debug, Default)]
struct ConnState {
    apps: HashMap<Pid, OsxApp>,
    windows: HashMap<WinId, OsxWindow>,
}

impl ConnState {
    fn update_known_apps_and_windows(&mut self) {
        let current_apps: HashMap<Pid, NSRunningApplication> = running_applications()
            .into_iter()
            .map(|app| (unsafe { app.processIdentifier() }, app))
            .collect();

        self.apps.retain(|k, _| current_apps.contains_key(k));
        for (pid, running_app) in current_apps.into_iter() {
            if !self.apps.contains_key(&pid) {
                if let Ok(app) = OsxApp::try_new(running_app) {
                    self.apps.insert(pid, app);
                }
            }
        }

        // Being lazy here for now, this should be pulling only the window ID out of the dicts and
        // using that to see if we need to pull the rest of the info when needed
        self.windows = OsxWindow::current_windows()
            .into_iter()
            .map(|win| (win.win_id, win))
            .collect();
    }

    fn win_id_for_axwin(&self, axwin: &AXUIElement) -> Option<WinId> {
        for win in self.windows.values() {
            if &win.axwin == axwin {
                return Some(win.win_id);
            }
        }

        None
    }

    fn win_prop<T>(&mut self, id: WinId, f: impl Fn(&OsxWindow) -> T) -> Result<T> {
        if !self.windows.contains_key(&id) {
            self.update_known_apps_and_windows();
        }
        self.windows
            .get(&id)
            .map(|win| f(win))
            .ok_or(Error::UnknownClient(id))
    }

    // More undocumented magic in the AX API...
    //  - https://github.com/koekeishiya/yabai/commit/3fe4c77b001e1a4f613c26f01ea68c0f09327f3a
    //  - https://github.com/rxhanson/Rectangle/pull/285
    fn with_suppressed_animations(
        &mut self,
        id: WinId,
        f: impl Fn(&OsxWindow) -> Result<()>,
    ) -> Result<()> {
        let win = win!(self, id)?;
        let app = self
            .apps
            .get(&win.owner_pid)
            .ok_or(custom_error!("unknown app pid {}", win.owner_pid))?;
        let was_enabled = app.enhanced_user_interface_enabled();
        if was_enabled {
            app.set_enhanced_user_interface(false)?;
        }
        let res = f(win);
        if was_enabled {
            app.set_enhanced_user_interface(true)?;
        }

        res
    }
}

impl ConnEvent for Event {
    fn requires_pointer_warp(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct OsxConn {
    conn_state: Mutex<ConnState>,
    rx: Receiver<Event>,
}

impl OsxConn {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        _ = EVENT_SENDER.set(tx);

        Self {
            conn_state: Default::default(),
            rx,
        }
    }

    pub fn init_wm_and_run(
        self,
        config: Config<Self>,
        key_bindings: KeyBindings<Self>,
        mouse_bindings: MouseBindings<Self>,
        mut init: impl FnMut(&mut WindowManager<Self>) -> Result<()> + Send + 'static,
    ) {
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

        spawn(move || {
            let mut wm = WindowManager::new(config, key_bindings, mouse_bindings, self).unwrap();
            init(&mut wm).unwrap();
            wm.run().unwrap()
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

    fn manage_new_windows(&self, state: &mut State<Self>) -> Result<()> {
        let ids: Vec<_> = self
            .conn_state
            .lock()
            .unwrap()
            .windows
            .values()
            .map(|win| win.win_id)
            .collect();

        for id in ids.iter() {
            if !state.client_set.contains(id) {
                self.manage(*id, state)?;
            }
        }

        Ok(())
    }

    fn focus_active_app_window(&self, pid: Pid, state: &mut State<Self>) -> Result<()> {
        let mut conn_state = self.conn_state.lock().unwrap();
        if let Some(id) = state.client_set.current_client() {
            let win = win!(conn_state, *id)?;
            if win.owner_pid == pid {
                return Ok(());
            }
        }
        let axwin = app!(conn_state, pid)?.focused_ax_window()?;
        if let Some(id) = conn_state.win_id_for_axwin(&axwin) {
            self.manage_new_windows(state)?;
            self.modify_and_refresh(state, |cs| cs.focus_client(&id))?;
        }

        Ok(())
    }

    fn clear_terminated_app_state(&self, pid: Pid, state: &mut State<Self>) -> Result<()> {
        let mut conn_state = self.conn_state.lock().unwrap();
        conn_state.apps.remove(&pid);
        let ids: Vec<_> = conn_state
            .windows
            .values()
            .flat_map(|w| {
                if w.owner_pid == pid {
                    Some(w.win_id)
                } else {
                    None
                }
            })
            .collect();
        conn_state.windows.retain(|_, win| win.owner_pid != pid);

        for id in ids.into_iter() {
            self.unmanage(id, state)?;
        }

        Ok(())
    }

    fn handle_app_hidden(&self, _pid: Pid, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }

    fn handle_app_unhidden(&self, _pid: Pid, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }

    fn clear_closed_window_state(&self, id: WinId, state: &mut State<Self>) -> Result<()> {
        let mut conn_state = self.conn_state.lock().unwrap();
        conn_state.windows.remove(&id);
        self.unmanage(id, state)
    }

    fn handle_window_position(&self, _id: WinId, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }

    fn handle_window_miniturized(&self, _id: WinId, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }

    fn handle_window_deminiturized(&self, _id: WinId, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }
}

impl Conn for OsxConn {
    type Event = Event;

    fn root(&self) -> WinId {
        ROOT
    }

    fn next_event(&self) -> Result<Self::Event> {
        self.rx.recv().map_err(|_| custom_error!("recv error"))
    }

    fn handle_event(
        &self,
        evt: Event,
        _key_bindings: &mut KeyBindings<Self>,
        _mouse_bindings: &mut MouseBindings<Self>,
        state: &mut State<Self>,
    ) -> Result<()> {
        use Event::*;
        debug!("got event: {evt:?}");

        match evt {
            AppActivated { pid } => self.focus_active_app_window(pid, state),
            AppDeactivated { .. } => Ok(()),
            AppLaunched { pid } => self.focus_active_app_window(pid, state),
            AppTerminated { pid } => self.clear_terminated_app_state(pid, state),
            AppHidden { pid } => self.handle_app_hidden(pid, state),
            AppUnhidden { pid } => self.handle_app_unhidden(pid, state),
            WindowCreated { pid } => self.focus_active_app_window(pid, state),
            UiElementDestroyed { id } => self.clear_closed_window_state(id, state),
            FocusedWindowChanged { pid } => self.focus_active_app_window(pid, state),
            WindowMoved { id } | WindowResized { id } => self.handle_window_position(id, state),
            WindowMiniturized { id } => self.handle_window_miniturized(id, state),
            WindowDeminiturized { id } => self.handle_window_deminiturized(id, state),
        }
    }

    fn flush(&self) {}

    fn grab(&self, _key_codes: &[KeyCode], _mouse_states: &[MouseState]) -> Result<()> {
        // TODO: actually grab keys and mouse states
        Ok(())
    }

    fn screen_details(&self) -> Result<Vec<Rect>> {
        let displays: Vec<_> = CGDisplay::active_displays()
            .map_err(|e| custom_error!("error reading cg displays: {}", e))?
            .into_iter()
            .map(|id| {
                let r = CGDisplay::new(id).bounds();
                Rect::new(
                    r.origin.x as i32,
                    r.origin.y as i32,
                    r.size.width as u32,
                    r.size.height as u32,
                )
            })
            .collect();

        Ok(displays)
    }

    fn cursor_position(&self) -> Result<Point> {
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| custom_error!("unable to get event source"))?;
        let p = CGEvent::new(source)
            .map_err(|_| custom_error!("unable to get point"))?
            .location();

        Ok(Point::new(p.x as i32, p.y as i32))
    }

    fn warp_pointer(&self, id: WinId, x: i16, y: i16) -> Result<()> {
        let p = if id == ROOT {
            CGPoint::new(x as f64, y as f64)
        } else {
            let mut state = self.conn_state.lock().unwrap();
            let r = state.win_prop(id, |win| win.bounds)?;
            CGPoint::new(r.x as f64 + x as f64, r.y as f64 + y as f64)
        };

        CGDisplay::warp_mouse_cursor_position(p)
            .map_err(|e| custom_error!("unable to warp cursor: {}", e))
    }

    fn existing_clients(&self) -> Result<Vec<WinId>> {
        let mut state = self.conn_state.lock().unwrap();
        state.update_known_apps_and_windows();

        Ok(state.windows.keys().cloned().collect())
    }

    fn position_client(&self, id: WinId, r: Rect) -> Result<()> {
        let mut state = self.conn_state.lock().unwrap();
        state.with_suppressed_animations(id, |win| {
            win.set_pos(r.x as f64, r.y as f64)?;
            win.set_size(r.w as f64, r.h as f64)
        })
    }

    // show/hide based on https://github.com/koekeishiya/yabai/blob/527b0aa7c259637138d3d7468b63e3a9eb742d30/src/window_manager.c#L2045

    fn show_client(&self, id: WinId) -> Result<()> {
        let mut state = self.conn_state.lock().unwrap();
        state.with_suppressed_animations(id, |win| {
            win.axwin
                .set_attribute(&AXAttribute::minimized(), false)
                .map_err(|e| custom_error!("error un-minimizing window: {}", e))
        })
    }

    fn hide_client(&self, id: WinId) -> Result<()> {
        let mut state = self.conn_state.lock().unwrap();
        state.with_suppressed_animations(id, |win| {
            win.axwin
                .set_attribute(&AXAttribute::minimized(), true)
                .map_err(|e| custom_error!("error minimizing window: {}", e))
        })
    }

    fn withdraw_client(&self, _id: WinId) -> Result<()> {
        Ok(()) // nothing to do
    }

    // based on https://github.com/koekeishiya/yabai/blob/527b0aa7c259637138d3d7468b63e3a9eb742d30/src/window_manager.c#L2066
    fn kill_client(&self, id: WinId) -> Result<()> {
        let mut state = self.conn_state.lock().unwrap();
        let win = match state.windows.get(&id) {
            Some(win) => win,
            None => {
                state.update_known_apps_and_windows();
                state.windows.get(&id).ok_or(Error::UnknownClient(id))?
            }
        };

        win.close()
    }

    fn focus_client(&self, id: WinId) -> Result<()> {
        let mut state = self.conn_state.lock().unwrap();
        let win = match state.windows.get(&id) {
            Some(win) => win,
            None => {
                state.update_known_apps_and_windows();
                state.windows.get(&id).ok_or(Error::UnknownClient(id))?
            }
        };
        let app = state.apps.get(&win.owner_pid).unwrap();

        win.raise()?;
        app.activate();

        Ok(())
    }

    fn client_geometry(&self, id: WinId) -> Result<Rect> {
        let mut state = self.conn_state.lock().unwrap();
        state.win_prop(id, |win| win.bounds)
    }

    fn client_title(&self, id: WinId) -> Result<String> {
        let mut state = self.conn_state.lock().unwrap();
        state.win_prop(id, |win| {
            win.window_name.clone().unwrap_or_else(|| win.owner.clone())
        })
    }

    fn client_pid(&self, id: WinId) -> Option<u32> {
        let mut state = self.conn_state.lock().unwrap();
        state.win_prop(id, |win| win.owner_pid as u32).ok()
    }

    fn client_should_float(&self, id: WinId, floating_classes: &[String]) -> bool {
        let mut state = self.conn_state.lock().unwrap();
        state
            .win_prop(id, |win| floating_classes.contains(&win.owner))
            .unwrap_or_default()
    }

    fn client_should_be_managed(&self, id: WinId) -> bool {
        // If we are able to pull in state for the window we should be managing it: the
        // construction of OsxWindow errors for windows we don't want to manage
        let mut state = self.conn_state.lock().unwrap();
        state.win_prop(id, |_| true).unwrap_or_default()
    }

    fn client_is_fullscreen(&self, id: WinId) -> bool {
        let mut state = self.conn_state.lock().unwrap();
        state
            .win_prop(id, |win| win.is_fullscreen())
            .unwrap_or_default()
    }

    fn client_transient_parent(&self, _id: WinId) -> Option<WinId> {
        None
    }

    fn set_client_border_color(&self, _id: WinId, _color: impl Into<Color>) -> Result<()> {
        Ok(()) // TODO: add support
    }

    fn set_client_border_width(&self, _id: WinId, _w: u32) -> Result<()> {
        Ok(()) // TODO: add support
    }

    fn set_initial_properties(&self, _id: WinId, _config: &Config<Self>) -> Result<()> {
        Ok(()) // nothing to do
    }

    fn restack<'a, I>(&self, _ids: I) -> Result<()>
    where
        WinId: 'a,
        I: Iterator<Item = &'a WinId>,
    {
        Ok(()) // TODO: add support
    }

    fn manage_existing_clients(&self, state: &mut State<Self>) -> Result<()> {
        info!("managing existing clients");
        self.conn_state
            .lock()
            .unwrap()
            .update_known_apps_and_windows();

        let current_idx = state.client_set.current_screen().index();
        self.manage_new_windows(state)?;
        state.client_set.focus_screen(current_idx);
        self.refresh(state)
    }
}
