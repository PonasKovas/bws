use bws_plugin_interface::{
    safe_types::*,
    vtable::{InitVTable, LogLevel, VTable},
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

pub static INIT_VTABLE: InitVTable = InitVTable {
    log,
    cmd_arg,
    cmd_flag,
    get_event_id,
    add_event_callback,
    remove_event_callback,
    stop_main_thread,
};

pub static VTABLE: VTable = VTable {
    log,
    get_cmd_arg,
    get_cmd_flag,
    get_event_id,
    add_event_callback,
    remove_event_callback,
    fire_event,
    get_plugin_vtable,
    stop_main_thread,
};

extern "C" fn log(target: SStr, level: LogLevel, message: SStr) {
    let level = match level {
        LogLevel::Error => log::Level::Error,
        LogLevel::Warn => log::Level::Warn,
        LogLevel::Info => log::Level::Info,
        LogLevel::Debug => log::Level::Debug,
        LogLevel::Trace => log::Level::Trace,
    };
    log::log!(target: target.as_str(), level, "{}", message.as_str());

    // If an error message is printed and log level is set to trace
    // print backtrace too
    if level == log::Level::Error {
        log::log!(target: target.as_str(), log::Level::Trace, "Backtrace:\n{}", std::backtrace::Backtrace::force_capture());
    }
}

extern "C" fn stop_main_thread() {
    *crate::END_PROGRAM.0.lock().unwrap() = true;
    crate::END_PROGRAM.1.notify_all();
}
