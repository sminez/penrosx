//! Minimal bindings to the accessibility APIs for OSX
//!
//! Adapted from <https://github.com/eiz/accessibility> which is not being actively maintained and
//! pulls in conflicting versions of crates that I need elsewhere.
//! The API defined here is an internal implementation detail and not suitable for general purpose
//! use.
use penrose::{Result, custom_error};
use std::mem::MaybeUninit;

pub mod actions;
pub mod attribute;
pub mod error;
pub mod notification;
pub mod ui_element;
pub mod value;

pub(crate) unsafe fn ax_call<F, V>(f: F) -> Result<V>
where
    F: Fn(*mut V) -> error::AXError,
{
    let mut result = MaybeUninit::uninit();
    let err = (f)(result.as_mut_ptr());

    if err.is_err() {
        return Err(custom_error!("AX error: {}", err));
    }

    Ok(unsafe { result.assume_init() })
}

pub(crate) unsafe fn ax_call_void<F>(f: F) -> Result<()>
where
    F: Fn() -> error::AXError,
{
    let err = (f)();
    if err.is_err() {
        return Err(custom_error!("AX error: {}", err));
    }

    Ok(())
}
