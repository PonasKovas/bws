use bws_plugin_interface::{
    safe_types::*,
    vtable::{InitVTable, LogLevel, VTable},
};
pub use cmd::*;
pub use plugin_api::*;

pub mod cmd;
pub mod plugin_api;

pub static INIT_VTABLE: InitVTable = InitVTable {
    log,
    cmd_arg,
    cmd_flag,
    stop_main_thread,
};

pub static VTABLE: VTable = VTable {
    log,
    get_cmd_arg,
    get_cmd_flag,
    get_plugin_vtable,
    stop_main_thread,
};

extern "C" fn log(plugin_id: usize, target: SStr, level: LogLevel, message: SStr) -> MaybePanicked {
    MaybePanicked::new(move || {
        let level = match level {
            LogLevel::Error => log::Level::Error,
            LogLevel::Warn => log::Level::Warn,
            LogLevel::Info => log::Level::Info,
            LogLevel::Debug => log::Level::Debug,
            LogLevel::Trace => log::Level::Trace,
        };
        log::log!(
            target:
                &(format!(
                    "[{}] {target}",
                    crate::plugins::PLUGINS.get().unwrap()[plugin_id]
                        .plugin
                        .name
                )),
            level,
            "{}",
            message
        );

        // If an error message is printed and log level is set to trace
        // print backtrace too
        if level == log::Level::Error {
            log::error!("Backtrace:\n{}", std::backtrace::Backtrace::force_capture());
        }
    })
}

extern "C" fn stop_main_thread(_plugin_id: usize) -> MaybePanicked {
    MaybePanicked::new(move || {
        *crate::END_PROGRAM.0.lock().unwrap() = true;
        crate::END_PROGRAM.1.notify_all();
    })
}
