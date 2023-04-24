use crate::{plugin_api::PluginApiPtr, safe_types::*};
use bws_plugin_api::PluginApi;
use std::fmt::Debug;

macro_rules! add_shared_functions {
    ($(#[$attrs:meta])* $pub:vis struct $name:ident { $( $(#[$fattrs:meta])* $fpub:vis $field:ident : $type:ty,)* }) => {
        $(#[$attrs])*
        $pub struct $name {
            /// Logs a message
            ///
            ///  - `target` - where the message is originating from (convention is to use `std::module_path!()`)
            ///  - `level` - the type of message
            ///  - `message` - the text
            pub log: extern "C" fn(plugin_id: usize, target: SStr, level: LogLevel, message: SStr) -> MaybePanicked,
            /// Ends the main thread, killing the whole process
            pub stop_main_thread: extern "C" fn(plugin_id: usize) -> MaybePanicked,

            $( $(#[$fattrs])* $fpub $field : $type,)*
        }
        impl $name {
            /// Logs a message
            pub fn log(&self, target: &str, level: LogLevel, message: &str) {
                (self.log)($crate::global::get_plugin_id(), target.into(), level, message.into()).unwrap();
            }
            /// Ends the main program thread, essentially stopping the process abruptly.
            pub fn stop_main_thread(&self) {
                (self.stop_main_thread)($crate::global::get_plugin_id()).unwrap();
            }
        }
    }
}

add_shared_functions! {
    /// This vtable is given access to in `init_fn`
    ///
    /// Allows to register command line arguments/flags
    #[repr(C)]
    pub struct InitVTable {
        /// Registers a command line argument for the application
        ///
        ///  - `id` - unique name for the argument, can be used later to retrieve the set value
        ///  - `short` - a `char` in the form of `u32` (example `'p' as u32`) defining the short way to set the argument
        ///  - `long` - the long way to set the argument
        ///  - `value_name` - the name/type of value that is expected (convention is to use all uppercase here)
        ///  - `help` - the help string
        ///  - `required` - whether the argument is mandatory
        ///
        /// The function will panic if:
        /// - `short` is not a valid `char`
        /// - `id` duplicates
        pub cmd_arg: extern "C" fn(plugin_id: usize, id: SStr, short: SOption<u32>, long: SStr, value_name: SStr, help: SStr, required: bool) -> MaybePanicked,
        /// Registers a command line flag for the application
        ///
        ///  - `id` - unique name for the flag, can be used later to check if it was set
        ///  - `short` - a `char` in the form of `u32` (example `'p' as u32`) defining the short way to set the flag
        ///  - `long` - the long way to set the flag
        ///  - `help` - the help string
        ///
        /// The function will panic if:
        /// - `short` is not a valid `char`
        /// - `id` duplicates
        pub cmd_flag: extern "C" fn(plugin_id: usize, id: SStr, short: SOption<u32>, long: SStr, help: SStr) -> MaybePanicked,
    }
}

add_shared_functions! {
    /// The main VTable for interacting with the host (BWS)
    ///
    /// Plugins are given static references to this in `vtable_fn`
    #[repr(C)]
    pub struct VTable {
        /// Retrieves a command line argument, if it was set
        ///
        ///  - `id` - the unique name of the argument. (Must match with the one given when registering the argument with `cmd_arg`)
        pub get_cmd_arg: extern "C" fn(plugin_id: usize, id: SStr) -> MaybePanicked<SOption<SString>>,
        /// Checks if a command line flag was set
        ///
        ///  - `id` - the unique name of the flag. (Must match with the one given when registering the flag with `cmd_flag`)
        pub get_cmd_flag: extern "C" fn(plugin_id: usize, id: SStr) -> MaybePanicked<bool>,
        /// Returns the `PluginApiPtr` of the requested plugin
        ///
        /// - `plugin_name` - the name of the plugin
        pub get_plugin_vtable: extern "C" fn(plugin_id: usize, plugin_name: SStr) -> MaybePanicked<PluginApiPtr>,
    }
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
    /// Attempts to retrieve a vtable exposed by another plugin.
    ///
    /// # Safety
    ///
    /// Will result in UB if used with an incorrect `API` type, which should not happen
    /// as long as the `PluginApi` trait is implemented correctly and the plugin that exposes
    /// the API uses semantic versioning correctly in the library defining the API.
    ///
    /// # Panics
    ///
    /// Panics if the requested plugin does not expose an API or incorrect API type used
    pub unsafe fn get_plugin_vtable<API: PluginApi>(&self, plugin_name: &str) -> &'static API {
        let vtable =
            (self.get_plugin_vtable)(crate::global::get_plugin_id(), plugin_name.into()).unwrap();

        unsafe { vtable.get() }
    }
    /// Retrieves a command line argument, if it was set.
    ///
    /// `id` is the `id` used in `InitVTable::cmd_arg`
    pub fn get_cmd_arg(&self, id: &str) -> Option<String> {
        (self.get_cmd_arg)(crate::global::get_plugin_id(), id.into())
            .unwrap()
            .into_option()
            .map(|s| s.into())
    }
    /// Checks if a command line flag was set
    ///
    /// `id` is the `id` used in `InitVTable::cmd_flag`
    pub fn get_cmd_flag(&self, id: &str) -> bool {
        (self.get_cmd_flag)(crate::global::get_plugin_id(), id.into()).unwrap()
    }
}

impl InitVTable {
    /// Registers a new command line argument for the application
    ///
    /// `id` is an unique name for the argument which can be used later to check the value.
    pub fn cmd_arg(
        &self,
        id: &str,
        short: Option<char>,
        long: &str,
        value_name: &str,
        help: &str,
        required: bool,
    ) {
        (self.cmd_arg)(
            crate::global::get_plugin_id(),
            id.into(),
            short.map(|x| x as u32).into(),
            long.into(),
            value_name.into(),
            help.into(),
            required,
        )
        .unwrap();
    }
    /// Registers a new command line flag for the application
    ///
    /// `id` is an unique name for the flag which can be used later to check if the flag was set.
    pub fn cmd_flag(&self, id: &str, short: Option<char>, long: &str, help: &str) {
        (self.cmd_flag)(
            crate::global::get_plugin_id(),
            id.into(),
            short.map(|x| x as u32).into(),
            long.into(),
            help.into(),
        )
        .unwrap();
    }
}

impl Debug for VTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VTable")
    }
}

impl Debug for InitVTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InitVTable")
    }
}
