use std::fmt::Debug;

use ironties::types::{FfiSafeEquivalent, MaybePanicked, SOption, SStr};

/// The main VTable for interacting with the host (BWS)
///
/// Plugins are given static references to this in `vtable_fn`
#[repr(C)]
pub struct VTable {
    /// Logs a message
    ///
    ///  - `target` - where the message is originating from (convention is to use `std::module_path!()`)
    ///  - `level` - the type of message
    ///  - `message` - the text
    pub log: extern "C" fn(
        plugin_id: usize,
        target: SStr,
        level: LogLevel,
        message: SStr,
    ) -> MaybePanicked,
    /// Ends the main thread, killing the whole process
    pub stop_main_thread: extern "C" fn(plugin_id: usize) -> MaybePanicked,
    /// Retrieves a command line argument, if it was set
    pub get_cmd_arg:
        extern "C" fn(plugin_id: usize, id: SStr) -> MaybePanicked<SOption<SStr<'static>>>,
    /// Checks if a command line flag was set
    pub get_cmd_flag: extern "C" fn(plugin_id: usize, id: SStr) -> MaybePanicked<bool>,
}

/// `#[repr(C)]` equivalent of `log::LogLevel`
#[repr(C)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl VTable {
    /// Logs a message
    pub fn log(&self, target: &str, level: LogLevel, message: &str) {
        (self.log)(
            crate::global::get_plugin_id(),
            target.into(),
            level,
            message.into(),
        )
        .unwrap();
    }
    /// Ends the main program thread, essentially stopping the process abruptly.
    pub fn stop_main_thread(&self) {
        (self.stop_main_thread)(crate::global::get_plugin_id()).unwrap();
    }
    /// Retrieves a command line argument, if it was set.
    ///
    /// `id` is the `id` used in `InitVTable::cmd_arg`
    pub fn get_cmd_arg(&self, id: &str) -> Option<&'static str> {
        (self.get_cmd_arg)(crate::global::get_plugin_id(), id.into())
            .unwrap()
            .into_normal()
            .map(|s| s.into_normal())
    }
    /// Checks if a command line flag was set
    ///
    /// `id` is the `id` used in `InitVTable::cmd_flag`
    pub fn get_cmd_flag(&self, id: &str) -> bool {
        (self.get_cmd_flag)(crate::global::get_plugin_id(), id.into()).unwrap()
    }
}

impl Debug for VTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VTable")
    }
}
