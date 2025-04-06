use anyhow::Context;
use penrose::{
    builtin::{
        actions::{modify_with, send_layout_message},
        layout::{
            MainAndStack,
            messages::{ExpandMain, IncMain, ShrinkMain},
            transformers::{Gaps, ReflectHorizontal},
        },
    },
    core::{Config, bindings::KeyEventHandler, layout::LayoutStack},
    map, stack,
};
use penrosx::conn::OsxConn;
use std::{collections::HashMap, io::stdout};
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

    OsxConn::new().init_wm_and_run(config, HashMap::default(), HashMap::default(), |_| Ok(()));

    Ok(())
}

fn raw_key_bindings() -> HashMap<String, Box<dyn KeyEventHandler<OsxConn>>> {
    let mut raw_bindings = map! {
        map_keys: |k: &str| k.to_owned();

        "M-j" => modify_with(|cs| cs.focus_down()),
        "M-k" => modify_with(|cs| cs.focus_up()),
        "M-S-j" => modify_with(|cs| cs.swap_down()),
        "M-S-k" => modify_with(|cs| cs.swap_up()),
        "M-S-q" => modify_with(|cs| cs.kill_focused()),
        "M-Tab" => modify_with(|cs| cs.toggle_tag()),
        "M-bracketright" => modify_with(|cs| cs.next_screen()),
        "M-bracketleft" => modify_with(|cs| cs.previous_screen()),
        "M-grave" => modify_with(|cs| cs.next_layout()),
        "M-S-grave" => modify_with(|cs| cs.previous_layout()),
        "M-Up" => send_layout_message(|| IncMain(1)),
        "M-Down" => send_layout_message(|| IncMain(-1)),
        "M-Right" => send_layout_message(|| ExpandMain),
        "M-Left" => send_layout_message(|| ShrinkMain),
    };

    for tag in &["1", "2", "3", "4", "5", "6", "7", "8", "9"] {
        raw_bindings.extend([
            (
                format!("M-{tag}"),
                modify_with(move |client_set| client_set.focus_tag(tag)),
            ),
            (
                format!("M-S-{tag}"),
                modify_with(move |client_set| client_set.move_focused_to_tag(tag)),
            ),
        ]);
    }

    raw_bindings
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
