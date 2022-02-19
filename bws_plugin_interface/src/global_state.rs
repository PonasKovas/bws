pub mod plugin;
pub mod vtable;

use abi_stable::{
    external_types::RRwLock,
    std_types::{RArc, RStr, RString, RVec, Tuple2},
};
use plugin::{Plugin, PluginList};
use std::fmt::Debug;

pub type GState = RArc<GlobalState>;

#[repr(C)]
pub struct GlobalState {
    pub plugins: RRwLock<PluginList>,
    pub vtable: vtable::VTable,
}
