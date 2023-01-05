#![allow(unused_imports)]

// TODO
//
// - Avoid UB when panicking across FFI.
//   Waiting for https://github.com/rust-lang/rust/issues/74990
//

#[cfg(feature = "plugin")]
pub mod macros;
pub mod plugin_api;
pub mod safe_types;
pub mod vtable;

pub use plugin_api::PluginApi;
use safe_types::*;
use vtable::{InitVTable, VTable};

// Incremented on each incompatible ABI change
pub const ABI: u64 = 17;

/// The main struct that all plugins should expose with the `BWS_PLUGIN_ROOT` name
///
/// # Example
///
/// ```ignore
/// #[no_mangle]
/// static BWS_PLUGIN_ROOT: BwsPlugin = BwsPlugin {
///     name: SStr::from_str(env!("CARGO_PKG_NAME")),
///     version: SStr::from_str(env!("CARGO_PKG_VERSION")),
///     dependencies: SSlice::from_slice(&[]),
///     on_load,
///     api: PluginApi::new(),
/// };
///
/// extern "C" fn on_load(vtable: &'static InitVTable) {
///     println!("Plugin initialization");
/// }
/// ...
/// ```
///
#[repr(C)]
#[derive(Debug)]
pub struct BwsPlugin {
    pub name: SStr<'static>,
    pub version: SStr<'static>,
    pub dependencies: SSlice<'static, STuple2<SStr<'static>, SStr<'static>>>,

    pub on_load: extern "C" fn(&'static InitVTable),

    pub api: PluginApi,
}
