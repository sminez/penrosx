use anyhow::Context;
use penrosx::{
    manager::WindowManager,
    state::{Config, State},
};
use tracing::subscriber::set_global_default;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    let builder = FmtSubscriber::builder()
        .with_env_filter("info")
        .with_writer(std::io::stdout);
    // .with_filter_reloading();

    let subscriber = builder.finish();

    set_global_default(subscriber).context("unable to set a global tracing subscriber")?;
    let state = State::try_new(Config::default())?;
    let mut wm = WindowManager::new(state);
    wm.refresh();

    // wm.run();

    Ok(())
}
