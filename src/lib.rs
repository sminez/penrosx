//! PenrOSX - An OSX backend for Penrose

#![warn(
    clippy::complexity,
    clippy::correctness,
    clippy::style,
    future_incompatible,
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    rustdoc::all,
    // clippy::undocumented_unsafe_blocks
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/sminez/penrose/develop/icon.svg",
    issue_tracker_base_url = "https://github.com/sminez/penrosx/issues/"
)]
pub use keyboard_types::{Code, Modifiers};
pub use objc2::MainThreadMarker;

mod app;
mod bindings;
mod conn;
mod event;
mod global_observer;
pub mod query;
mod sys;
mod win;

pub use bindings::{HotKey, try_parse_key_bindings};
pub use conn::{OsxConn, ROOT};
pub use event::Event;
pub use sys::ax::{actions::AXUIElementActions, attribute::AXUIElementAttributes, error::AXError};
