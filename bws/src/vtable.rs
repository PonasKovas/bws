use bws_plugin_interface::{
    safe_types::*,
    vtable::{LogLevel, VTable},
    PluginApi,
};
pub use cmd::*;
pub use event::*;
use once_cell::sync::{Lazy, OnceCell};
pub use plugin_api::*;
use std::sync::Mutex;

pub mod cmd;
pub mod event;
pub mod plugin_api;

pub static VTABLE: VTable = VTable {
    log,
    cmd_arg,
    cmd_flag,
    get_cmd_arg,
    get_cmd_flag,
    get_event_id,
    add_event_callback,
    fire_event,
    get_plugin_vtable,
};

/// Logs a message
///
///  - `target` - where the message is originating from (convention is to use `std::module_path!()`)
///  - `level` - the type of message
///  - `message` - the text
extern "C" fn log(target: SStr, level: LogLevel, message: SStr) {
    let level = match level {
        LogLevel::Error => log::Level::Error,
        LogLevel::Warn => log::Level::Warn,
        LogLevel::Info => log::Level::Info,
        LogLevel::Debug => log::Level::Debug,
        LogLevel::Trace => log::Level::Trace,
    };
    log::log!(target: target.as_str(), level, "{}", message.as_str());
}
