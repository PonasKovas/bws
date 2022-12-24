#![allow(unused_imports)]

// TODO
//
// - Avoid UB when panicking across FFI.
//   Waiting for https://github.com/rust-lang/rust/issues/74990
//

pub mod global_state;
#[cfg(feature = "plugin")]
pub mod macros;
pub mod plugin_api;
pub mod safe_types;
pub mod vtable;

pub use plugin_api::PluginApi;
use safe_types::*;
use vtable::VTable;

// Incremented on each incompatible ABI change
pub const ABI: u64 = 15;

/// The main struct that all plugins should expose with the `BWS_PLUGIN_ROOT` name
///
/// # Example
///
/// ```ignore
/// #[no_mangle]
/// static BWS_PLUGIN_ROOT: BwsPlugin = BwsPlugin {
///     name: RStr::from_str("plugin_template"),
///     version: RStr::from_str(env!("CARGO_PKG_VERSION")),
///     dependencies: RSlice::from_slice(&[]),
///     on_load,
///     ...
/// };
///
/// fn on_load(gstate: &GState) {
///     println!("Plugin template enabled");
/// }
/// ...
/// ```
///
#[repr(C)]
pub struct BwsPlugin {
    pub name: SStr<'static>,
    pub version: SStr<'static>,
    pub dependencies: SSlice<'static, STuple2<SStr<'static>, SStr<'static>>>,

    pub on_load: extern "C" fn(&'static VTable),

    pub api: PluginApi,
}
