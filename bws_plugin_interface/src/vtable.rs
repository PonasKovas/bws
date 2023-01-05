use crate::{safe_types::*, PluginApi};

macro_rules! add_shared_functions {
    ($(#[$attrs:meta])* $pub:vis struct $name:ident { $( $(#[$fattrs:meta])* $fpub:vis $field:ident : $type:ty,)* }) => {
        $(#[$attrs])*
        $pub struct $name {
            /// Logs a message
            ///
            ///  - `target` - where the message is originating from (convention is to use `std::module_path!()`)
            ///  - `level` - the type of message
            ///  - `message` - the text
            pub log: extern "C" fn(target: SStr, level: LogLevel, message: SStr),
            /// Returns the numerical ID of the event, which will stay the same for the lifetime of the process
            ///
            /// - `event_name` - the string name of the event
            pub get_event_id: extern "C" fn(SStr) -> usize,
            /// Registers a callback for an event
            ///
            ///  - `event_id` - the numerical ID of the event (can be obtained with `get_event_id`)
            ///  - `plugin_name` - the name of the plugin which is registering the callback
            ///  - `callback` - the callback function pointer
            ///  - `priority` - the priority of this callback. Default is 0.0, the callbacks are executed in the order of their priority in ascending order
            ///
            /// Note: an event can't have multiple callbacks with the same function pointer, it acts like an ID for a callback
            /// and can be used to remove the callback (remove_event_callback)
            pub add_event_callback: extern "C" fn(usize, SStr, EventFn, f64),
            /// Removes a callback from an event
            ///
            ///  - `event_id` - the numerical ID of the event (can be obtained with `get_event_id`)
            ///  - `callback` - the callback function pointer
            pub remove_event_callback: extern "C" fn(usize, EventFn),
            /// Ends the main program thread, essentially stopping the process abruptly.
            pub stop_main_thread: extern "C" fn(),

            $( $(#[$fattrs])* $fpub $field : $type,)*
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
        pub cmd_arg: extern "C" fn(SStr, u32, SStr, SStr, SStr, bool),
        /// Registers a command line flag for the application
        ///
        ///  - `id` - unique name for the flag, can be used later to check if it was set
        ///  - `short` - a `char` in the form of `u32` (example `'p' as u32`) defining the short way to set the flag
        ///  - `long` - the long way to set the flag
        ///  - `help` - the help string
        ///
        /// The function will panic if `short` is not a valid `char`
        pub cmd_flag: extern "C" fn(SStr, u32, SStr, SStr),
    }
}

add_shared_functions! {
    #[repr(C)]
    pub struct VTable {
        /// Retrieves a command line argument, if it was set
        ///
        ///  - `id` - the unique name of the argument. (Must match with the one given when registering the argument with `cmd_arg`)
        pub get_cmd_arg: extern "C" fn(SStr) -> SOption<SString>,
        /// Checks if a command line flag was set
        ///
        ///  - `id` - the unique name of the flag. (Must match with the one given when registering the flag with `cmd_flag`)
        pub get_cmd_flag: extern "C" fn(SStr) -> bool,
        /// Fires an event and executes the callbacks associated
        ///
        ///  - `event_id` - the numerical ID of the event (can be obtained with `get_event_id`)
        ///  - `data` - a pointer to arbitrary data that event handlers will have access to
        ///
        /// Returns `false` if the event handling was ended by a callback, `true` otherwise.
        ///
        /// Note: the `start` event has hardcoded special behaviour: it's called automatically once the program
        /// is ready and can not be called again.
        pub fire_event: extern "C" fn(usize, *const ()) -> bool,
        /// Returns the PluginApi of the requested plugin
        ///
        /// - `plugin_name` - the name of the plugin
        pub get_plugin_vtable: extern "C" fn(SStr) -> PluginApi,
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

/// Takes arbitrary data behind a pointer and must return a boolean
/// `true` means to continue the event, `false` - to end it
/// and not call any further event fns that are in queue for this
/// specific instance of event
pub type EventFn = extern "C" fn(&'static VTable, *const ()) -> bool;
