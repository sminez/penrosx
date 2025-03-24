use penrosx::{
    ax::{proc_is_ax_trusted, register_observers, set_ax_timeout},
    win::{OsxApp, OsxWindow},
};

fn main() {
    println!("proc is trusted: {}", proc_is_ax_trusted());
    set_ax_timeout();
    register_observers();

    let apps = OsxApp::running_applications();
    for app in apps.iter() {
        println!("{app:?}");
    }

    let wins = OsxWindow::current_windows();
    for win in wins.iter() {
        println!("{win:?}");

        // if win.owner == "Slack" {
        //     if let Err(err) = win.set_pos(-1201.0, 80.0) {
        //         println!("{err}");
        //     };
        //     if let Err(err) = win.set_size(400.0, 200.0) {
        //         println!("{err}");
        //     };
        // }
    }

    println!("sleeping");
    std::thread::sleep(std::time::Duration::from_secs(10));
    println!("exiting");
}
