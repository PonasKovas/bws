use safer_ffi::derive_ReprC;
use safer_ffi::slice::slice_ref;
use safer_ffi::string::str_ref;
use safer_ffi::tuple::Tuple2;

#[derive_ReprC]
#[repr(C)]
pub struct BwsPlugin {
    pub name: str_ref<'static>,
    pub version: str_ref<'static>,
    pub dependencies: slice_ref<'static, Tuple2<str_ref<'static>, str_ref<'static>>>,
    pub api: Option<plugin_api::PluginApiPtr>,
}

mod macros;
pub mod plugin_api;
pub mod vtable;

pub use bws_plugin_api::PluginApi;

use vtable::{InitVTable, VTable};

pub const ABI: &'static str = env!("CARGO_PKG_VERSION");

// pub use global::get_vtable;

// #[doc(hidden)]
// pub mod global {
//     use once_cell::sync::OnceCell;

//     static PLUGIN_ID: OnceCell<usize> = OnceCell::new();
//     static VTABLE: OnceCell<&'static crate::VTable> = OnceCell::new();

//     pub fn get_plugin_id() -> usize {
//         *PLUGIN_ID.get().expect("plugin id global not set")
//     }
//     /// Returns a reference to the [`VTable`][crate::VTable] that's saved in a static
//     ///
//     /// # Panics
//     ///
//     /// Panics if static not initialized yet.
//     pub fn get_vtable() -> &'static crate::VTable {
//         VTABLE.get().expect("vtable global not set")
//     }
//     pub fn set_plugin_id(id: usize) {
//         PLUGIN_ID.set(id).expect("plugin id already set");
//     }
//     pub fn set_vtable(vtable: &'static crate::VTable) {
//         VTABLE.set(vtable).expect("vtable already set");
//     }
// }
