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
use accessibility::AXUIElement;
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
        conn::{Conn, ConnEvent, ConnExt, manage_without_refresh},
    },
    custom_error,
    pure::geometry::{Point, Rect},
};
use std::{
    collections::HashMap,
    sync::mpsc::{Receiver, Sender, channel},
    thread::spawn,
};
use tracing::{debug, error, info, trace, warn};

const ROOT: WinId = WinId(0);

macro_rules! win_mut {
    ($self:ident, $id:expr) => {
        match $self.windows.get_mut(&$id) {
            Some(win) => Ok(win),
            None => {
                $self.update_known_apps_and_windows();
                $self.windows.get_mut(&$id).ok_or(Error::UnknownClient($id))
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

impl ConnEvent for Event {
    fn requires_pointer_warp(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct OsxConn {
    apps: HashMap<Pid, OsxApp>,
    windows: HashMap<WinId, OsxWindow>,
    hide_pt: Point,
    rx: Receiver<Event>,
}

impl OsxConn {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        _ = EVENT_SENDER.set(tx);

        Self {
            apps: Default::default(),
            windows: Default::default(),
            hide_pt: Default::default(),
            rx,
        }
    }

    /// Get a copy of the sender required to inject events into the connection event stream
    pub fn event_tx(&self) -> Sender<Event> {
        EVENT_SENDER.get().unwrap().clone()
    }

    pub fn init_wm_and_run(
        mut self,
        config: Config<Self>,
        key_bindings: KeyBindings<Self>,
        mouse_bindings: MouseBindings<Self>,
        init: impl FnOnce(&mut WindowManager<Self>) -> Result<()> + Send + 'static,
    ) {
        if !proc_is_ax_trusted() {
            panic!("process is not trusted for the AX API");
        }

        set_ax_timeout();
        self.set_hide_pt().unwrap();

        let (_pool, app) = unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let app = NSApp();
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

            (pool, app)
        };

        spawn(move || {
            let mut wm = WindowManager::new(config, key_bindings, mouse_bindings, self).unwrap();
            init(&mut wm).unwrap();
            wm.run().unwrap();
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

    fn set_hide_pt(&mut self) -> Result<()> {
        let r_last_screen = self
            .screen_details()?
            .into_iter()
            .last()
            .ok_or(Error::NoScreens)?;

        self.hide_pt = Point::new(
            r_last_screen.x + r_last_screen.w as i32 - 1,
            r_last_screen.y + r_last_screen.h as i32 - 1,
        );

        Ok(())
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
        f: impl Fn(&mut OsxWindow) -> Result<()>,
    ) -> Result<()> {
        let win = win_mut!(self, id)?;
        let app = self
            .apps
            .get_mut(&win.owner_pid)
            .ok_or(custom_error!("unknown app pid {}", win.owner_pid))?;
        let mut was_enabled = app.enhanced_user_interface_enabled();
        if was_enabled {
            if app.set_enhanced_user_interface(false).is_err() {
                was_enabled = false; // avoid trying to reset
            }
        }
        let res = f(win);
        if was_enabled {
            _ = app.set_enhanced_user_interface(true);
        }

        res
    }

    fn manage_new_windows(&mut self, state: &mut State<Self>) -> Result<()> {
        let ids: Vec<_> = self.windows.values().map(|win| win.win_id).collect();

        for id in ids.iter() {
            if !state.client_set.contains(id) {
                self.manage(*id, state)?;
            }
        }

        Ok(())
    }

    fn focus_active_app_window(&mut self, pid: Pid, state: &mut State<Self>) -> Result<()> {
        let axwin = match app!(self, pid)?.focused_ax_window() {
            Ok(axwin) => axwin,
            Err(_) => return Ok(()), // if we can't find the window then skip
        };
        let maybe_id = self.win_id_for_axwin(&axwin);
        if state.client_set.current_client() == maybe_id.as_ref() {
            return Ok(()); // already focused
        }
        if let Some(id) = maybe_id {
            self.manage_new_windows(state)?;
            self.modify_and_refresh(state, |cs| cs.focus_client(&id))?;
        }

        Ok(())
    }

    fn clear_terminated_app_state(&mut self, pid: Pid, state: &mut State<Self>) -> Result<()> {
        self.apps.remove(&pid);
        let ids: Vec<_> = self
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
        self.windows.retain(|_, win| win.owner_pid != pid);

        for id in ids.into_iter() {
            self.unmanage(id, state)?;
        }

        Ok(())
    }

    fn handle_new_window_for_pid(&mut self, pid: Pid, state: &mut State<Self>) -> Result<()> {
        let old_ids: Vec<WinId> = self.windows.keys().map(|id| *id).collect();
        self.update_known_apps_and_windows();
        let new_windows: Vec<_> = self
            .windows
            .values()
            .filter(|w| w.owner_pid == pid && !old_ids.contains(&w.win_id))
            .map(|w| w.win_id)
            .collect();

        if new_windows.is_empty() {
            warn!(%pid, "WindowCreated fired but no new windows for pid were found");
            return Ok(());
        }

        debug!(?new_windows, "handling new window(s) for pid");
        let focus = *new_windows.last().unwrap();
        for id in new_windows.into_iter() {
            self.manage(id, state)?;
        }

        self.modify_and_refresh(state, |cs| cs.focus_client(&focus))
    }

    fn handle_app_hidden(&mut self, _pid: Pid, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }

    fn handle_app_unhidden(&mut self, _pid: Pid, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }

    fn clear_closed_window_state(&mut self, id: WinId, state: &mut State<Self>) -> Result<()> {
        self.windows.remove(&id);
        self.unmanage(id, state)
    }

    fn handle_window_position(&mut self, _id: WinId, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }

    fn handle_window_miniturized(&mut self, _id: WinId, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }

    fn handle_window_deminiturized(&mut self, _id: WinId, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }

    fn handle_keypress(
        &mut self,
        key: KeyCode,
        bindings: &mut KeyBindings<Self>,
        state: &mut State<Self>,
    ) -> Result<()> {
        if let Some(action) = bindings.get_mut(&key) {
            trace!(?key, "running user keybinding");
            if let Err(error) = action.call(state, self) {
                error!(%error, ?key, "error running user keybinding");
                return Err(error);
            }
        }

        Ok(())
    }
}

impl Conn for OsxConn {
    type Event = Event;

    fn root(&mut self) -> WinId {
        ROOT
    }

    fn next_event(&mut self) -> Result<Self::Event> {
        self.rx.recv().map_err(|_| custom_error!("recv error"))
    }

    fn handle_event(
        &mut self,
        evt: Event,
        key_bindings: &mut KeyBindings<Self>,
        _mouse_bindings: &mut MouseBindings<Self>,
        state: &mut State<Self>,
    ) -> Result<()> {
        use Event::*;

        match evt {
            AppActivated { pid } => self.focus_active_app_window(pid, state),
            AppLaunched { pid } => self.focus_active_app_window(pid, state),
            FocusedWindowChanged { pid } => self.focus_active_app_window(pid, state),

            AppHidden { pid } => self.handle_app_hidden(pid, state),
            AppTerminated { pid } => self.clear_terminated_app_state(pid, state),
            AppUnhidden { pid } => self.handle_app_unhidden(pid, state),
            UiElementDestroyed { id } => self.clear_closed_window_state(id, state),
            WindowCreated { pid } => self.handle_new_window_for_pid(pid, state),
            WindowDeminiturized { id } => self.handle_window_deminiturized(id, state),
            WindowMiniturized { id } => self.handle_window_miniturized(id, state),
            WindowMoved { id } | WindowResized { id } => self.handle_window_position(id, state),

            KeyPress { k } => self.handle_keypress(k, key_bindings, state),

            AppDeactivated { .. } => Ok(()),
        }
    }

    fn flush(&mut self) {}

    fn grab(&mut self, _key_codes: &[KeyCode], _mouse_states: &[MouseState]) -> Result<()> {
        // TODO: actually grab keys and mouse states
        Ok(())
    }

    fn screen_details(&mut self) -> Result<Vec<Rect>> {
        let mut displays: Vec<_> = CGDisplay::active_displays()
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

        displays.sort_by_key(|r| r.x);

        Ok(displays)
    }

    fn cursor_position(&mut self) -> Result<Point> {
        let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .map_err(|_| custom_error!("unable to get event source"))?;
        let p = CGEvent::new(source)
            .map_err(|_| custom_error!("unable to get point"))?
            .location();

        Ok(Point::new(p.x as i32, p.y as i32))
    }

    fn warp_pointer(&mut self, id: WinId, x: i16, y: i16) -> Result<()> {
        let p = if id == ROOT {
            CGPoint::new(x as f64, y as f64)
        } else {
            let r = self.win_prop(id, |win| win.bounds)?;
            CGPoint::new(r.x as f64 + x as f64, r.y as f64 + y as f64)
        };

        CGDisplay::warp_mouse_cursor_position(p)
            .map_err(|e| custom_error!("unable to warp cursor: {}", e))?;

        if id != ROOT {
            self.focus_client(id)?;
        }

        Ok(())
    }

    fn existing_clients(&mut self) -> Result<Vec<WinId>> {
        self.update_known_apps_and_windows();

        Ok(self.windows.keys().cloned().collect())
    }

    fn position_client(&mut self, id: WinId, r: Rect) -> Result<()> {
        self.with_suppressed_animations(id, |win| {
            win.set_pos(r.x as f64, r.y as f64)?;
            win.set_size(r.w as f64, r.h as f64)?;
            win.bounds = r;
            Ok(())
        })
    }

    fn show_client(&mut self, _id: WinId, _state: &mut State<Self>) -> Result<()> {
        Ok(())
    }

    fn hide_client(&mut self, id: WinId, _state: &mut State<Self>) -> Result<()> {
        let p = self.hide_pt;
        self.with_suppressed_animations(id, |win| {
            win.set_pos(p.x as f64, p.y as f64)?;
            win.bounds.x = p.x;
            win.bounds.y = p.y;
            Ok(())
        })
    }

    fn withdraw_client(&mut self, _id: WinId) -> Result<()> {
        Ok(()) // nothing to do
    }

    // based on https://github.com/koekeishiya/yabai/blob/527b0aa7c259637138d3d7468b63e3a9eb742d30/src/window_manager.c#L2066
    fn kill_client(&mut self, id: WinId) -> Result<()> {
        let win = match self.windows.get(&id) {
            Some(win) => win,
            None => {
                self.update_known_apps_and_windows();
                self.windows.get(&id).ok_or(Error::UnknownClient(id))?
            }
        };

        win.close()
    }

    fn focus_client(&mut self, id: WinId) -> Result<()> {
        let win = match self.windows.get(&id) {
            Some(win) => win,
            None => {
                self.update_known_apps_and_windows();
                self.windows.get(&id).ok_or(Error::UnknownClient(id))?
            }
        };
        let app = self.apps.get(&win.owner_pid).unwrap();

        win.raise()?;
        app.activate();

        Ok(())
    }

    fn client_geometry(&mut self, id: WinId) -> Result<Rect> {
        self.win_prop(id, |win| win.bounds)
    }

    fn client_title(&mut self, id: WinId) -> Result<String> {
        self.win_prop(id, |win| {
            win.window_name.clone().unwrap_or_else(|| win.owner.clone())
        })
    }

    fn client_pid(&mut self, id: WinId) -> Option<u32> {
        self.win_prop(id, |win| win.owner_pid as u32).ok()
    }

    fn client_should_float(&mut self, id: WinId, floating_classes: &[String]) -> bool {
        self.win_prop(id, |win| floating_classes.contains(&win.owner))
            .unwrap_or_default()
    }

    fn client_should_be_managed(&mut self, id: WinId) -> bool {
        self.win_prop(id, |win| win.window_layer == 0)
            .unwrap_or_default()
    }

    fn client_is_fullscreen(&mut self, id: WinId) -> bool {
        self.win_prop(id, |win| win.is_fullscreen())
            .unwrap_or_default()
    }

    fn client_transient_parent(&mut self, _id: WinId) -> Option<WinId> {
        None
    }

    // https://github.com/cmacrae/limelight/blob/master/src/main.c#L200

    fn set_client_border_color(&mut self, _id: WinId, _color: impl Into<Color>) -> Result<()> {
        Ok(()) // TODO: add support
    }

    fn set_initial_properties(&mut self, _id: WinId, _config: &Config<Self>) -> Result<()> {
        Ok(()) // nothing to do
    }

    fn restack<'a, I>(&mut self, _ids: I) -> Result<()>
    where
        WinId: 'a,
        I: Iterator<Item = &'a WinId>,
    {
        Ok(()) // TODO: add support
    }

    fn manage_existing_clients(&mut self, state: &mut State<Self>) -> Result<()> {
        self.update_known_apps_and_windows();
        let to_check: Vec<_> = self
            .windows
            .iter()
            .map(|(id, win)| (*id, win.bounds.midpoint()))
            .collect();

        let screens = self.screen_details()?;
        info!(?to_check, ?screens, "windows to check");

        for (id, p) in to_check.into_iter() {
            if !state.client_set.contains(&id) && self.client_should_be_managed(id) {
                let tag = state
                    .client_set
                    .screens()
                    .find(|s| s.geometry().contains_point(p))
                    .map(|s| s.workspace.tag().to_owned());

                info!(%id, ?tag, "attempting to manage existing client");
                manage_without_refresh(id, tag.as_deref(), state, self)?;
            }
        }

        info!("triggering refresh");
        self.refresh(state)
    }
}
