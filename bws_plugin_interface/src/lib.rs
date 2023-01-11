#![allow(unused_imports)]

pub mod macros;
pub mod plugin_api;
pub mod vtable;

#[cfg(feature = "plugin")]
pub use bws_plugin_api::PluginApi;

pub use safe_types;
use safe_types::*;
use vtable::{InitVTable, VTable};

/// Incremented on each incompatible ABI change
pub const ABI: u64 = 19 | (safe_types::ABI as u64) << 32;

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

    pub init_fn: extern "C" fn(usize, &'static InitVTable) -> MaybePanicked<SResult>,
    pub start_fn: extern "C" fn(&'static VTable) -> MaybePanicked<SResult>,

    pub api: SOption<plugin_api::PluginApiPtr>,
}

#[cfg(feature = "plugin")]
#[doc(hidden)]
pub mod global {
    use std::{
        cell::UnsafeCell,
        mem::MaybeUninit,
        sync::atomic::{AtomicI8, Ordering},
    };

    pub struct SetOnce<T> {
        // -1 = not set
        //  0 = setting
        // +1 =     set
        state: AtomicI8,
        data: UnsafeCell<MaybeUninit<T>>,
    }
    unsafe impl<T: Sync> Sync for SetOnce<T> {}

    impl<T: Sync> SetOnce<T> {
        pub const fn new() -> Self {
            Self {
                state: AtomicI8::new(-1),
                data: UnsafeCell::new(MaybeUninit::uninit()),
            }
        }
        /// returns `false` if failed
        pub fn set(&self, data: T) -> bool {
            let old = self.state.swap(0, Ordering::Relaxed);
            if old == -1 {
                unsafe { self.data.get().write(MaybeUninit::new(data)) };
                self.state.store(1, Ordering::Release);
                true
            } else if old == 0 {
                false
            } else {
                self.state.store(old, Ordering::Release);
                false
            }
        }
        /// returns `None` if not set yet
        pub fn get(&self) -> Option<&T> {
            if self.state.load(Ordering::Acquire) == 1 {
                Some(unsafe {
                    self.data
                        .get()
                        .as_ref()
                        .unwrap_unchecked()
                        .assume_init_ref()
                })
            } else {
                None
            }
        }
    }

    pub static PLUGIN_ID: SetOnce<usize> = SetOnce::new();
    pub static VTABLE: SetOnce<&'static crate::VTable> = SetOnce::new();

    pub fn get_plugin_id() -> usize {
        *PLUGIN_ID.get().expect("plugin id global not set")
    }
    pub fn get_vtable() -> &'static crate::VTable {
        VTABLE.get().expect("vtable global not set")
    }
}
