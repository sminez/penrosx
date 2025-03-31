use penrosx::{manager::WindowManager, state::Config};

fn main() {
    let wm = WindowManager::new(Config::default());

    wm.run();
}
