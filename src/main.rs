use anyhow::Context;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use penrose::{
    builtin::{
        actions::{modify_with, send_layout_message},
        layout::{
            MainAndStack,
            messages::{ExpandMain, IncMain, ShrinkMain},
            transformers::{Gaps, ReflectHorizontal},
        },
    },
    core::{
        Config,
        bindings::{KeyBindings, KeyCode, KeyEventHandler},
        layout::LayoutStack,
    },
    map, stack,
};
use penrosx::{conn::OsxConn, sys::Event};
use std::{collections::HashMap, io::stdout, sync::mpsc::Sender};
use tracing::subscriber::set_global_default;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    let builder = FmtSubscriber::builder()
        .with_env_filter("trace")
        .with_writer(stdout);
    let subscriber = builder.finish();
    set_global_default(subscriber).context("unable to set a global tracing subscriber")?;

    let config = Config {
        default_layouts: layouts(),
        ..Config::default()
    };

    let conn = OsxConn::new();
    let (_manager, key_bindings) = register_global_hotkeys(conn.event_tx())?;
    conn.init_wm_and_run(config, key_bindings, HashMap::default(), |_| Ok(()));

    Ok(())
}

fn layouts() -> LayoutStack {
    let max_main = 1;
    let ratio = 0.6;
    let ratio_step = 0.1;
    let outer_px = 5;
    let inner_px = 5;

    stack!(
        MainAndStack::side(max_main, ratio, ratio_step),
        ReflectHorizontal::wrap(MainAndStack::side(max_main, ratio, ratio_step)),
        MainAndStack::bottom(max_main, ratio, ratio_step)
    )
    .map(|layout| Gaps::wrap(layout, outer_px, inner_px))
}

fn raw_key_bindings() -> HashMap<String, Box<dyn KeyEventHandler<OsxConn>>> {
    let mut raw_bindings = map! {
        map_keys: |k: &str| k.to_owned();

        "Super+j" => modify_with(|cs| cs.focus_down()),
        "Super+k" => modify_with(|cs| cs.focus_up()),
        "Super+Shift+j" => modify_with(|cs| cs.swap_down()),
        "Super+Shift+k" => modify_with(|cs| cs.swap_up()),
        "Super+Shift+q" => modify_with(|cs| cs.kill_focused()),
        "Super+Tab" => modify_with(|cs| cs.toggle_tag()),
        "Super+bracketright" => modify_with(|cs| cs.next_screen()),
        "Super+bracketleft" => modify_with(|cs| cs.previous_screen()),
        "Super+Shift+bracketright" => modify_with(|cs| cs.drag_workspace_forward()),
        "Super+Shift+bracketleft" => modify_with(|cs| cs.drag_workspace_backward()),
        "Super+backquote" => modify_with(|cs| cs.next_layout()),
        "Super+Shift+backquote" => modify_with(|cs| cs.previous_layout()),
        "Super+Up" => send_layout_message(|| IncMain(1)),
        "Super+Down" => send_layout_message(|| IncMain(-1)),
        "Super+Right" => send_layout_message(|| ExpandMain),
        "Super+Left" => send_layout_message(|| ShrinkMain),
    };

    for tag in &["1", "2", "3", "4", "5", "6", "7", "8", "9"] {
        raw_bindings.extend([
            (
                format!("Super+{tag}"),
                modify_with(move |client_set| client_set.focus_tag(tag)),
            ),
            (
                format!("Super+Shift+{tag}"),
                modify_with(move |client_set| client_set.move_focused_to_tag(tag)),
            ),
        ]);
    }

    raw_bindings
}

fn register_global_hotkeys(
    tx: Sender<Event>,
) -> anyhow::Result<(GlobalHotKeyManager, KeyBindings<OsxConn>)> {
    let hotkeys_manager = GlobalHotKeyManager::new()?;
    let raw = raw_key_bindings();

    let mut bindings = HashMap::with_capacity(raw.len());
    let mut rev_map = HashMap::with_capacity(raw.len());

    // using synthetic key codes internally because we just need to look them up in a map
    for (i, (s, handler)) in raw.into_iter().enumerate() {
        let hotkey = HotKey::try_from(s.as_str())?;
        let k = KeyCode {
            mask: 0,
            code: i as u8,
        };
        rev_map.insert(hotkey.id, k);
        hotkeys_manager.register(hotkey)?;
        bindings.insert(k, handler);
    }

    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.state == HotKeyState::Pressed {
            if let Some(k) = rev_map.get(&event.id) {
                let _ = tx.send(Event::KeyPress { k: *k });
            }
        }
    }));

    Ok((hotkeys_manager, bindings))
}
