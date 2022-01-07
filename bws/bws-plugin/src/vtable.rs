use crate::{
    prelude::*,
    register::{_f_PluginEntry, _f_SubPluginEntry},
};
use async_ffi::{FfiContext, FfiPoll};

#[repr(C)]
#[derive(Clone)]
pub struct BwsVTable {
    /// Takes:
    /// 1. A pointer to the registrator (given in the lib init function)
    /// 2. Plugin's name
    /// 3. Semver version (major.minor.patch)
    /// 4. Plugin's dependencies [(Name, VersionRequirement)]
    /// 5. A list of subscribed events
    /// 6. Entry point for the plugin
    ///
    /// Returns an index of the plugin for adding subplugins
    pub register_plugin: unsafe extern "C" fn(
        *mut (),
        BwsStr,
        BwsTuple3<u64, u64, u64>,
        BwsSlice<BwsTuple2<BwsStr, BwsStr>>,
        BwsSlice<u32>,
        _f_PluginEntry,
    ) -> usize,
    /// Takes:
    /// 1. A pointer to the registrator (given in the lib init function)
    /// 2. Index of the plugin (from register_plugin)
    /// 3. Subplugin's name
    /// 4. A list of subscribed events
    /// 5. Entry point for the subplugin.
    pub register_subplugin:
        unsafe extern "C" fn(*mut (), usize, BwsStr, BwsSlice<u32>, _f_SubPluginEntry),
    /// Takes:
    /// 1. A pointer to the receiver
    /// 2. FfiContext reference
    ///
    /// Returns:
    /// `None` if the channel is dead and no more events can be received.
    /// A plugin event and a pointer to the oneshot channel for signaling end of event handling.
    pub recv_plugin_event: unsafe extern "C" fn(
        *const (),
        &mut FfiContext,
    ) -> FfiPoll<
        BwsOption<BwsTuple2<BwsEvent<'static>, *const ()>>,
    >,
    /// Takes:
    /// 1. A pointer to the sender
    pub send_oneshot: unsafe extern "C" fn(*const ()),
    /// Takes a pointer of the global state Arc and drops it
    pub drop_global_state: unsafe extern "C" fn(*const ()),
    /// Takes a pointer and returns the compression treshold set in global state
    pub gs_get_compression_treshold: unsafe extern "C" fn(*const ()) -> i32,
    /// Takes a pointer and returns the port set in global state
    pub gs_get_port: unsafe extern "C" fn(*const ()) -> u16,
}

pub(crate) static mut VTABLE: BwsVTable = {
    // It's probably UB to use this single function with all these different signatures, but I don't care
    unsafe extern "C" fn not_set() {
        panic!("VTable not set. Hint: make sure to bws_plugin::vtable::init() before using any methods.");
    }

    unsafe {
        BwsVTable {
            register_plugin: std::mem::transmute(not_set as *const ()),
            register_subplugin: std::mem::transmute(not_set as *const ()),
            recv_plugin_event: std::mem::transmute(not_set as *const ()),
            send_oneshot: std::mem::transmute(not_set as *const ()),
            drop_global_state: std::mem::transmute(not_set as *const ()),
            gs_get_compression_treshold: std::mem::transmute(not_set as *const ()),
            gs_get_port: std::mem::transmute(not_set as *const ()),
        }
    }
};

pub fn init(vtable: &'static BwsVTable) {
    unsafe {
        VTABLE = vtable.clone();
    }
}
