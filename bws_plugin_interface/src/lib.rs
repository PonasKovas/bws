pub const ABI: SStr<'static> = SStr::new(env!("CARGO_PKG_VERSION"));

use ironties::{
    types::{MaybePanicked, SOption, SSlice, SStr, STuple2, SUnit},
    TypeLayout,
};
use std::fmt::Debug;

mod macros;
mod vtable;

pub use ironties;
pub use vtable::{LogLevel, VTable};

pub use global::get_vtable as vtable;

#[repr(C)]
pub struct BwsPlugin {
    pub name: SStr<'static>,
    pub depends_on: SSlice<'static, STuple2<SStr<'static>, SStr<'static>>>,
    pub provides: SSlice<'static, Api>,
    pub cmd: SSlice<'static, Cmd>,
    pub start: extern "C" fn(plugin_id: usize, vtable: &'static VTable) -> MaybePanicked<SUnit>,
}

#[repr(C)]
pub struct Api {
    pub name: SStr<'static>,
    pub version: SStr<'static>,
    pub vtable: *const (),
    pub vtable_layout: extern "C" fn() -> MaybePanicked<TypeLayout>,
}

#[repr(C)]
pub struct Cmd {
    /// Unique ID for the flag/argument
    pub id: SStr<'static>,
    pub short: SOption<char>,
    pub long: SStr<'static>,
    pub help: SStr<'static>,
    pub kind: CmdKind,
}

#[repr(C)]
pub enum CmdKind {
    Argument {
        value_name: SStr<'static>,
        required: bool,
    },
    Flag,
}

#[doc(hidden)]
pub mod global {
    use once_cell::sync::OnceCell;

    static PLUGIN_ID: OnceCell<usize> = OnceCell::new();
    static VTABLE: OnceCell<&'static crate::VTable> = OnceCell::new();

    pub fn get_plugin_id() -> usize {
        *PLUGIN_ID.get().expect("plugin id global not set")
    }
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

impl Debug for BwsPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BwsPlugin {:?}", self.name)
    }
}
