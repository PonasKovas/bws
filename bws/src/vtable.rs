// pub const VTABLE: VTable = VTable { log };

// fn log(target: RStr, level: LogLevel, message: RStr) {
//     let level = match level {
//         LogLevel::Error => log::Level::Error,
//         LogLevel::Warn => log::Level::Warn,
//         LogLevel::Info => log::Level::Info,
//         LogLevel::Debug => log::Level::Debug,
//         LogLevel::Trace => log::Level::Trace,
//     };
//     log::log!(target: target.as_str(), level, "{}", message.as_str());
// }
