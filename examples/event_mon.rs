use anyhow::Context;
use penrosx::{MainThreadMarker, OsxConn};
use std::io::stdout;
use tracing::info;
use tracing::subscriber::set_global_default;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    let builder = FmtSubscriber::builder()
        .with_env_filter("trace")
        .with_writer(stdout);
    let subscriber = builder.finish();
    set_global_default(subscriber).context("unable to set a global tracing subscriber")?;

    let mtm = MainThreadMarker::new().unwrap();
    let conn = OsxConn::try_new(mtm)?;
    conn.run_with_event_handler(mtm, |evt, conn| {
        info!(?evt, "got event");
        conn.update_known_apps_and_windows();

        info!(
            apps=?conn.apps().values().map(|w|w.string_details()).collect::<Vec<_>>(),
            "known apps"
        );
        info!(
            windows=?conn.windows().values().map(|w|w.string_details()).collect::<Vec<_>>(),
            "known windows"
        );
    });

    Ok(())
}
