use penrosx::a11y::{current_windows, proc_is_ax_trusted, register_observers, set_ax_timeout};
use penrosx::nsworkspace::{
    INSArray, INSRunningApplication, INSWorkspace, NSArray, NSRunningApplication,
    NSString_NSStringDeprecated, NSWorkspace, NSWorkspace_NSWorkspaceRunningApplications,
};

fn main() {
    println!("proc is trusted: {}", proc_is_ax_trusted());
    set_ax_timeout();
    // test_windows();
    unsafe {
        let arr = NSWorkspace::sharedWorkspace().runningApplications();
        let count = <NSArray as INSArray<NSRunningApplication>>::count(&arr);
        for i in 0..count {
            let app = NSRunningApplication(
                <NSArray as INSArray<NSRunningApplication>>::objectAtIndex_(&arr, i),
            );
            if app.activationPolicy() != 0 {
                continue;
            }
            let name = app.localizedName();
            let cstr = name.cString();
            let s = std::ffi::CStr::from_ptr(cstr);
            println!("{i} {s:?}");
        }
    }
}

fn test_windows() {
    let wins = current_windows().unwrap();
    for win in wins.into_iter() {
        println!("{} {}", win.owner, win.window_layer);
        // if win.owner == "Slack" {
        //     if let Err(err) = win.set_pos(-1201.0, 80.0) {
        //         println!("{err}");
        //     };
        //     if let Err(err) = win.set_size(400.0, 200.0) {
        //         println!("{err}");
        //     };
        // }
    }

    register_observers();

    println!("sleeping");
    std::thread::sleep(std::time::Duration::from_secs(100));
    println!("exiting");
}
