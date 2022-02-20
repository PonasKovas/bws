#![allow(unused_imports)]

// TODO
//
// - Avoid UB when panicking across FFI.
//   Waiting for https://github.com/rust-lang/rust/issues/74990
//

pub mod extra;
pub mod global_state;
#[cfg(feature = "plugin")]
pub mod macros;
pub mod stream;

use abi_stable::{
    sabi_types::{RMut, VersionStrings},
    std_types::{RArc, RBox, RBoxError, RCow, RResult, RSlice, RStr, RString, RVec, Tuple2},
};
use extra::Extra;
use global_state::GState;

pub const ABI: u64 = 14 | (safe_types::ABI_VERSION as u64) << 32;

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
///     enable,
///     disable,
///     ...
/// };
///
/// fn enable(gstate: &GState) {
///     println!("Plugin template enabled");
/// }
/// fn disable(_gstate: &GState) {
///     println!("Plugin template disabled");
/// }
/// ...
/// ```
///
#[repr(C)]
pub struct BwsPlugin {
    pub name: RStr<'static>,
    pub version: RStr<'static>,
    pub dependencies: RSlice<'static, Tuple2<RStr<'static>, RStr<'static>>>,

    pub enable: fn(gstate: &GState),
    pub disable: fn(gstate: &GState),

    pub extra: Extra,
}
