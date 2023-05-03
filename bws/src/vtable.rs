use bws_plugin_interface::{ironties::types::*, LogLevel, VTable};

pub static VTABLE: VTable = VTable {
    log,
    get_cmd_arg,
    get_cmd_flag,
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
                &format!(
                    "[{}] {target}",
                    crate::plugins::PLUGINS.get().unwrap()[plugin_id]
                        .plugin
                        .name
                ),
            level,
            "{}",
            message
        );

        // If an error message is printed and log level is set to trace
        // print backtrace too
        if level == log::Level::Error && log::log_enabled!(log::Level::Trace) {
            log::trace!("Backtrace:\n{}", std::backtrace::Backtrace::force_capture());
        }

        SUnit::new()
    })
}

extern "C" fn stop_main_thread(plugin_id: usize) -> MaybePanicked {
    MaybePanicked::new(move || {
        log::debug!(
            "Shutdown issued by {}",
            crate::plugins::PLUGINS.get().unwrap()[plugin_id]
                .plugin
                .name
        );
        let _ = crate::END_PROGRAM.set(());

        SUnit::new()
    })
}

extern "C" fn get_cmd_arg(plugin_id: usize, id: SStr) -> MaybePanicked<SOption<SStr<'static>>> {
    MaybePanicked::new(move || {
        log::debug!(
            "Plugin {} queried cmd arg {}",
            crate::plugins::PLUGINS.get().unwrap()[plugin_id]
                .plugin
                .name,
            id
        );

        crate::cmd::get_arg(id.into_normal())
            .map(|s| s.into())
            .into()
    })
}
extern "C" fn get_cmd_flag(plugin_id: usize, id: SStr) -> MaybePanicked<bool> {
    MaybePanicked::new(move || {
        log::debug!(
            "Plugin {} queried cmd flag {}",
            crate::plugins::PLUGINS.get().unwrap()[plugin_id]
                .plugin
                .name,
            id
        );

        crate::cmd::get_flag(id.into_normal())
    })
}
