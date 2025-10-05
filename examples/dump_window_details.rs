use penrose::core::conn::Query;
use penrosx::{
    AXUIElementAttributes, MainThreadMarker, OsxConn,
    query::{AppName, Title},
};

fn main() -> anyhow::Result<()> {
    let mtm = MainThreadMarker::new().unwrap();
    let mut conn = OsxConn::try_new(mtm)?;
    conn.update_known_apps_and_windows();

    let q = AppName("kitty").and(Title("scratchpad"));
    let mut ids = Vec::new();

    for win in conn.windows().values() {
        println!("id: {}", win.id());
        println!(
            "app name: {}",
            conn.app_for_window(win.id())?.axui_elem().title()?
        );
        println!("owner: {}", win.owner());
        println!("title: {}", win.axui_elem().title()?);
        println!("window name: {:?}", win.window_name());
        println!();
        ids.push(win.id());
    }

    for id in ids {
        println!("matches query {id}: {}", q.run(id, &mut conn)?);
    }

    Ok(())
}
