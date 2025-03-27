use penrosx::{
    ax::{proc_is_ax_trusted, register_observers, set_ax_timeout},
    state::{Config, State},
};

fn main() {
    println!("proc is trusted: {}", proc_is_ax_trusted());
    set_ax_timeout();
    register_observers();

    let state = State::try_new(Config::default()).unwrap();
    println!("{state:#?}");

    println!("sleeping");
    std::thread::sleep(std::time::Duration::from_secs(10));
    println!("exiting");
}
