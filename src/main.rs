use penrosx::{
    ax::{proc_is_ax_trusted, set_ax_timeout},
    state::{Config, State},
};

fn main() {
    println!("proc is trusted: {}", proc_is_ax_trusted());
    set_ax_timeout();

    println!("constructing state");
    let state = State::try_new(Config::default()).unwrap();
    // println!("{state:#?}");

    state.run();
}
