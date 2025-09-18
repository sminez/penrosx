pub mod conn;

#[allow(
    unsafe_op_in_unsafe_fn,
    non_upper_case_globals,
    non_camel_case_types,
    improper_ctypes,
    unexpected_cfgs,
    non_snake_case,
    clippy::all,
    dead_code,
    clippy::not_unsafe_ptr_arg_deref,
    reason = "generated code"
)]
#[expect(
    unnecessary_transmutes,
    reason = "bindgen codegen under Rust 1.88+ - https://github.com/rust-lang/rust-bindgen/issues/3241"
)]
pub(crate) mod nsworkspace;

pub mod sys;
pub mod win;
