use bws_plugin_interface::{
    safe_types::*,
    vtable::{LogLevel, VTable},
};

pub static VTABLE: VTable = VTable { log };

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
