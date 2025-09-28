//! This is the initial demo of the Conn impl from this crate working in a minimal way.
//! There is still a lot to sort out and keybindings need to be handled internally rather than
//! pulling in the global_hotkey crate but it's a start.
use anyhow::Context;
use penrose::{
    builtin::{
        actions::{modify_with, send_layout_message},
        layout::{
            MainAndStack,
            messages::{ExpandMain, IncMain, ShrinkMain},
            transformers::ReflectHorizontal,
        },
    },
    core::{Config, bindings::KeyBindings, layout::LayoutStack},
    map, stack,
};
use penrosx::{MainThreadMarker, conn::OsxConn, try_parse_key_bindings};
use std::{collections::HashMap, io::stdout};
use tracing::subscriber::set_global_default;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    let builder = FmtSubscriber::builder()
        .with_env_filter("debug")
        .with_writer(stdout);
    let subscriber = builder.finish();
    set_global_default(subscriber).context("unable to set a global tracing subscriber")?;

    let config = Config {
        default_layouts: layouts(),
        ..Config::default()
    };

    let conn = OsxConn::try_new()?;
    let mtm = MainThreadMarker::new().unwrap();
    conn.init_wm_and_run(mtm, config, key_bindings()?, HashMap::default(), |_| Ok(()));

    Ok(())
}

fn layouts() -> LayoutStack {
    let max_main = 1;
    let ratio = 0.6;
    let ratio_step = 0.1;

    stack!(
        MainAndStack::side(max_main, ratio, ratio_step),
        ReflectHorizontal::wrap(MainAndStack::side(max_main, ratio, ratio_step)),
        MainAndStack::bottom(max_main, ratio, ratio_step)
    )
}

fn key_bindings() -> penrose::Result<KeyBindings<OsxConn>> {
    let mut raw_bindings = map! {
        map_keys: |k: &str| k.to_owned();

        "M-j" => modify_with(|cs| cs.focus_down()),
        "M-k" => modify_with(|cs| cs.focus_up()),
        "M-S-j" => modify_with(|cs| cs.swap_down()),
        "M-S-k" => modify_with(|cs| cs.swap_up()),
        "M-S-q" => modify_with(|cs| cs.kill_focused()),
        "M-A-Tab" => modify_with(|cs| cs.toggle_tag()),
        "M-bracketright" => modify_with(|cs| cs.next_screen()),
        "M-bracketleft" => modify_with(|cs| cs.previous_screen()),
        "M-S-bracketright" => modify_with(|cs| cs.drag_workspace_forward()),
        "M-S-bracketleft" => modify_with(|cs| cs.drag_workspace_backward()),
        "M-backquote" => modify_with(|cs| cs.next_layout()),
        "M-S-backquote" => modify_with(|cs| cs.previous_layout()),
        "M-up" => send_layout_message(|| IncMain(1)),
        "M-down" => send_layout_message(|| IncMain(-1)),
        "M-right" => send_layout_message(|| ExpandMain),
        "M-left" => send_layout_message(|| ShrinkMain),
    };

    for tag in &["1", "2", "3", "4", "5", "6", "7", "8", "9"] {
        raw_bindings.extend([
            (
                format!("M-{tag}"),
                modify_with(move |client_set| client_set.focus_tag(tag)),
            ),
            (
                format!("M-A-{tag}"),
                modify_with(move |client_set| client_set.move_focused_to_tag(tag)),
            ),
        ]);
    }

    try_parse_key_bindings(raw_bindings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_bindings_are_valid() {
        let res = key_bindings();
        assert!(res.is_ok(), "{res:?}")
    }
}
