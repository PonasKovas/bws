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
pub mod vtable;

use plugin_api::PluginApi;

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
    // pub name: RStr<'static>,
    // pub version: RStr<'static>,
    // pub dependencies: RSlice<'static, Tuple2<RStr<'static>, RStr<'static>>>,

    // pub on_load: fn(gstate: &GState),

    // pub extra: Extra,
}
