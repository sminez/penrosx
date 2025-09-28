use anyhow::Context;
use penrosx::{MainThreadMarker, conn::OsxConn};
use std::io::stdout;
use tracing::subscriber::set_global_default;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    let builder = FmtSubscriber::builder()
        .with_env_filter("trace")
        .with_writer(stdout);
    let subscriber = builder.finish();
    set_global_default(subscriber).context("unable to set a global tracing subscriber")?;

    let conn = OsxConn::try_new()?;
    conn.log_incoming_events(MainThreadMarker::new().unwrap());

    Ok(())
}
