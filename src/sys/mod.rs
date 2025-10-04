use core_graphics::display::CGDisplay;
use objc2_app_kit::{NSScreen, NSWindow};
use objc2_foundation::MainThreadMarker;
use penrose::pure::geometry::Rect;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) mod ax;
pub(crate) mod carbon;

static HAVE_CREATED_WINDOW: AtomicBool = AtomicBool::new(false);

/// An application process ID.
pub type Pid = i32;

pub(crate) fn current_screen_rects(mtm: MainThreadMarker) -> Vec<Rect> {
    if !HAVE_CREATED_WINDOW.load(Ordering::Relaxed) {
        // https://stackoverflow.com/questions/29953638/nsscreen-visibleframe-only-accounting-for-menu-bar-area-on-main-screen
        let _ = unsafe { NSWindow::new(mtm) };
        HAVE_CREATED_WINDOW.store(true, Ordering::Relaxed);
    }

    let main_bounds = CGDisplay::main().bounds();
    let main_y = (main_bounds.origin.y + main_bounds.size.height) as i32;

    NSScreen::screens(mtm)
        .into_iter()
        .map(|screen| {
            let vframe = NSScreen::visibleFrame(&screen);
            Rect::new(
                vframe.origin.x as i32,
                main_y - (vframe.origin.y + vframe.size.height) as i32,
                vframe.size.width as u32,
                vframe.size.height as u32,
            )
        })
        .collect()
}
