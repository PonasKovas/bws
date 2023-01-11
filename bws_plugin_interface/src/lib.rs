#![allow(unused_imports)]

#[cfg(feature = "plugin")]
mod macros;
pub mod plugin_api;
pub mod vtable;

#[cfg(feature = "plugin")]
pub use bws_plugin_api::PluginApi;

pub use safe_types;
use safe_types::*;
use vtable::{InitVTable, VTable};

/// Incremented on each incompatible ABI change
pub const ABI: u64 = 20 | (safe_types::ABI as u64) << 32;

/// The main struct that all plugins should expose with the `BWS_PLUGIN_ROOT` name
///
/// # Example
///
/// ```
/// # use bws_plugin_interface::{vtable::{VTable, InitVTable}, info, BwsPlugin};
/// # use safe_types::{MaybePanicked, SResult, SUnit, SOption, SStr, SSlice};
/// #[no_mangle]
/// static BWS_PLUGIN_ROOT: BwsPlugin = BwsPlugin {
///     name: SStr::from_str(env!("CARGO_PKG_NAME")),
///     version: SStr::from_str(env!("CARGO_PKG_VERSION")),
///     dependencies: SSlice::from_slice(&[]),
///     init_fn,
///     vtable_fn,
///     start_fn,
///     api: SOption::None,
/// };
///
/// extern "C" fn init_fn(my_id: usize, vtable: &'static InitVTable) -> MaybePanicked<SResult> {
///     MaybePanicked::new(move || {
///         bws_plugin_interface::global::set_plugin_id(my_id);
///
///         info!(vtable, "Plugin initialization");
///
///         SResult::Ok(SUnit::new())
///     })
/// }
///
/// extern "C" fn vtable_fn(vtable: &'static VTable) -> MaybePanicked {
///     MaybePanicked::new(move || {
///         bws_plugin_interface::global::set_vtable(vtable);
///
///         SUnit::new()
///     })
/// }
///
/// extern "C" fn start_fn() -> MaybePanicked<SResult> {
///     MaybePanicked::new(move || {
///         info!("Plugin start");
///
///         SResult::Ok(SUnit::new())
///     })
/// }
/// ```
///
#[repr(C)]
#[derive(Debug)]
pub struct BwsPlugin {
    pub name: SStr<'static>,
    pub version: SStr<'static>,
    pub dependencies: SSlice<'static, STuple2<SStr<'static>, SStr<'static>>>,

    pub init_fn: extern "C" fn(usize, &'static InitVTable) -> MaybePanicked<SResult>,
    pub vtable_fn: extern "C" fn(&'static VTable) -> MaybePanicked,
    pub start_fn: extern "C" fn() -> MaybePanicked<SResult>,

    pub api: SOption<plugin_api::PluginApiPtr>,
}

#[cfg(feature = "plugin")]
pub use global::get_vtable;

#[cfg(feature = "plugin")]
#[doc(hidden)]
pub mod global {
    use once_cell::sync::OnceCell;

    static PLUGIN_ID: OnceCell<usize> = OnceCell::new();
    static VTABLE: OnceCell<&'static crate::VTable> = OnceCell::new();

    pub fn get_plugin_id() -> usize {
        *PLUGIN_ID.get().expect("plugin id global not set")
    }
    /// Returns a reference to the [`VTable`][crate::VTable] that's saved in a static
    ///
    /// # Panics
    ///
    /// Panics if static not initialized yet.
    pub fn get_vtable() -> &'static crate::VTable {
        VTABLE.get().expect("vtable global not set")
    }
    pub fn set_plugin_id(id: usize) {
        PLUGIN_ID.set(id).expect("plugin id already set");
    }
    pub fn set_vtable(vtable: &'static crate::VTable) {
        VTABLE.set(vtable).expect("vtable already set");
    }
}
