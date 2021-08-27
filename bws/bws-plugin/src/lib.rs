#![allow(non_camel_case_types)]

pub mod events;
pub mod pointers;
pub mod register;
pub mod stable_types;
pub mod vtable;

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct SendMutPtr<T>(pub *mut T);

unsafe impl<T> Send for SendMutPtr<T> {}
unsafe impl<T> Sync for SendMutPtr<T> {}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct SendPtr<T>(pub *const T);

unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

/// A gate for the plugin side, bundles all that is required to handle events for sub-plugins
#[repr(C)]
pub struct BwsSubPluginGate {
    receiver: *const (),
}

pub mod prelude {
    pub use crate::events::BwsEvent;
    pub use crate::pointers::{global_state::BwsGlobalState, plugin_gate::BwsPluginGate};
    pub use crate::stable_types::{
        option::BwsOption,
        slice::BwsSlice,
        string::{BwsStr, BwsString},
        tuples::{BwsTuple2, BwsTuple3, BwsTuple4, BwsTuple5},
        unit::{unit, BwsUnit},
        vec::BwsVec,
    };
    pub use crate::vtable::BwsVTable;
    pub use crate::{SendMutPtr, SendPtr};
}
