use anyhow::Context;
use penrose::core::Config;
use penrosx::conn::OsxConn;
use std::collections::HashMap;
use tracing::subscriber::set_global_default;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    let builder = FmtSubscriber::builder()
        .with_env_filter("trace")
        .with_writer(std::io::stdout);
    let subscriber = builder.finish();
    set_global_default(subscriber).context("unable to set a global tracing subscriber")?;

    OsxConn::new().init_wm_and_run(
        Config::default(),
        HashMap::default(),
        HashMap::default(),
        |_| Ok(()),
    );

    Ok(())
}
