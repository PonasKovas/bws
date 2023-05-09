use crate::global::get_plugin_id;
use ironties::{
    types::{FfiSafeEquivalent, MaybePanicked, SOption, SStr, STuple2},
    TypeInfo, TypeLayout,
};
use std::fmt::Debug;

/// The main VTable for interacting with the host (BWS)
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
    /// Returns a pointer to a vtable provided by another plugin and it's type layout
    pub get_vtable: extern "C" fn(
        plugin_id: usize,
        vtable: SStr,
    ) -> MaybePanicked<STuple2<*const (), TypeLayout>>,
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
        (self.log)(get_plugin_id(), target.into(), level, message.into()).unwrap();
    }
    /// Ends the main program thread, essentially stopping the process abruptly.
    pub fn stop_main_thread(&self) {
        (self.stop_main_thread)(get_plugin_id()).unwrap();
    }
    /// Retrieves a command line argument, if it was set.
    ///
    /// `id` is the `id` used in `InitVTable::cmd_arg`
    pub fn get_cmd_arg(&self, id: &str) -> Option<&'static str> {
        (self.get_cmd_arg)(get_plugin_id(), id.into())
            .unwrap()
            .into_normal()
            .map(|s| s.into_normal())
    }
    /// Checks if a command line flag was set
    ///
    /// `id` is the `id` used in `InitVTable::cmd_flag`
    pub fn get_cmd_flag(&self, id: &str) -> bool {
        (self.get_cmd_flag)(get_plugin_id(), id.into()).unwrap()
    }
    /// Retrieves a vtable from another plugin.
    pub fn get_vtable<T: TypeInfo>(&self, name: &str) -> &'static T {
        let STuple2(ptr, layout) = (self.get_vtable)(get_plugin_id(), name.into()).unwrap();

        assert_eq!(
            layout,
            T::layout(),
            "VTable layout does not match! Make sure you're using the correct type and version."
        );

        unsafe { (ptr as *const T).as_ref() }.unwrap()
    }
}

impl Debug for VTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VTable")
    }
}
