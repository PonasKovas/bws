use crate::{safe_types::*, PluginApi};

#[repr(C)]
pub struct VTable {
    pub log: extern "C" fn(target: SStr, level: LogLevel, message: SStr),
    pub cmd_arg: extern "C" fn(SStr, u32, SStr, SStr, SStr, bool),
    pub cmd_flag: extern "C" fn(SStr, u32, SStr, SStr),
    pub get_cmd_arg: extern "C" fn(SStr) -> SOption<SString>,
    pub get_cmd_flag: extern "C" fn(SStr) -> bool,
    pub get_event_id: extern "C" fn(SStr) -> usize,
    pub add_event_callback: extern "C" fn(usize, SStr, EventFn, SSlice<SStr>),
    pub fire_event: extern "C" fn(usize, *const ()) -> bool,
    pub get_plugin_vtable: extern "C" fn(SStr) -> PluginApi,
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
