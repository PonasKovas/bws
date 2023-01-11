use bws_plugin_api::PluginApi;

use crate::{plugin_api::PluginApiPtr, safe_types::*};

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
            pub stop_main_thread: extern "C" fn(plugin_id: usize) -> MaybePanicked,

            $( $(#[$fattrs])* $fpub $field : $type,)*
        }
        impl $name {
            /// Ends the main program thread, essentially stopping the process abruptly.
            pub fn stop_main_thread(&self) {
                (self.stop_main_thread)($crate::global::get_plugin_id()).unwrap();
            }
        }
    }
}

add_shared_functions! {
    /// This vtable is given access to in `on_load`
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
        /// The function will panic if `short` is not a valid `char`
        pub cmd_arg: extern "C" fn(plugin_id: usize, id: SStr, short: u32, long: SStr, value_name: SStr, help: SStr, required: bool) -> MaybePanicked,
        /// Registers a command line flag for the application
        ///
        ///  - `id` - unique name for the flag, can be used later to check if it was set
        ///  - `short` - a `char` in the form of `u32` (example `'p' as u32`) defining the short way to set the flag
        ///  - `long` - the long way to set the flag
        ///  - `help` - the help string
        ///
        /// The function will panic if `short` is not a valid `char`
        pub cmd_flag: extern "C" fn(plugin_id: usize, id: SStr, short: u32, long: SStr, help: SStr) -> MaybePanicked,
    }
}

add_shared_functions! {
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

#[repr(C)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl VTable {
    /// # Safety
    ///
    /// panics if plugin does not expose an API or incorrect API type used
    pub unsafe fn get_plugin_vtable<API: PluginApi>(&self, plugin_name: &str) -> &'static API {
        (self.get_plugin_vtable)(crate::global::get_plugin_id(), plugin_name.into())
            .unwrap()
            .get()
    }
}
