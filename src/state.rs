//! OSX specific state that we track
use crate::{
    ax::running_applications,
    nsworkspace::{INSRunningApplication, NSRunningApplication},
    win::{OsxApp, OsxWindow, Pid, WinId},
};
use core_graphics::display::{CGDisplay, CGRect};
use penrose::{
    Color, Result,
    core::layout::LayoutStack,
    custom_error,
    pure::{Diff, StackSet, geometry::Rect},
};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Config {
    /// The RGBA color to use for normal (unfocused) window borders
    pub normal_border: Color,
    /// The RGBA color to use for the focused window border
    pub focused_border: Color,
    /// The width in pixels to use for drawing window borders
    pub border_width: u32,
    /// Whether or not the mouse entering a new window should set focus
    pub focus_follow_mouse: bool,
    /// The stack of layouts to use for each workspace
    pub default_layouts: LayoutStack,
    /// The ordered set of workspace tags to use on window manager startup
    pub tags: Vec<String>,
    /// Window classes that should always be assigned floating positions rather than tiled
    pub floating_classes: Vec<String>,
    // TODO: hooks
}

impl Default for Config {
    fn default() -> Self {
        let strings = |slice: &[&str]| slice.iter().map(|s| s.to_string()).collect();

        Config {
            normal_border: "#3c3836ff".try_into().expect("valid hex code"),
            focused_border: "#cc241dff".try_into().expect("valid hex code"),
            border_width: 2,
            focus_follow_mouse: true,
            default_layouts: LayoutStack::default(),
            tags: strings(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]),
            floating_classes: strings(&["dmenu", "dunst"]),
        }
    }
}

/// Unlike under X11, we need to maintain some more state on our side to be able to interact with
/// windows (or pull that state for every interaction) so we need to store and update maps of
/// running applications and associated windows.
#[derive(Debug)]
pub struct State {
    pub config: Config,
    pub stack_set: StackSet<WinId>,
    pub apps: HashMap<Pid, OsxApp>,
    pub windows: HashMap<WinId, OsxWindow>,
    pub diff: Diff<WinId>,
}

impl State {
    pub fn try_new(config: Config) -> Result<Self> {
        let mut display_rects: Vec<_> = cg_displays()?
            .into_iter()
            .map(|r| {
                Rect::new(
                    r.origin.x as i32,
                    r.origin.y as i32,
                    r.size.width as u32,
                    r.size.height as u32,
                )
            })
            .collect();
        display_rects.sort_by_key(|r| (r.x, r.y));

        let mut stack_set = StackSet::try_new(
            config.default_layouts.clone(),
            config.tags.iter(),
            display_rects,
        )?;

        let ss = stack_set.snapshot(vec![]);
        let diff = Diff::new(ss.clone(), ss);

        let mut state = Self {
            config,
            stack_set,
            apps: HashMap::new(),
            windows: HashMap::new(),
            diff,
        };

        // TODO: updating this state will potentially invalidate the IDs held within the stack set
        // so we also need to prune from there if things have been added or removed
        state.update_known_apps_and_windows();

        Ok(state)
    }

    pub(crate) fn update_known_apps_and_windows(&mut self) {
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
}

fn cg_displays() -> Result<Vec<CGRect>> {
    let displays: Vec<_> = CGDisplay::active_displays()
        .map_err(|e| custom_error!("error reading cg displays: {}", e))?
        .into_iter()
        .map(|id| CGDisplay::new(id).bounds())
        .collect();

    Ok(displays)
}
