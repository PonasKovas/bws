#![allow(non_camel_case_types)]

pub mod events;
pub mod pointers;
pub mod register;
pub mod stable_types;
pub mod vtable;

use async_ffi::{ContextExt, FfiContext, FfiFuture, FfiPoll};
pub use events::{PluginEvent, SubPluginEvent};
use pointers::{global_state::BwsGlobalState, plugin_gate::PluginGate};
pub use stable_types::{
    option::BwsOption,
    slice::BwsSlice,
    string::{BwsStr, BwsString},
    tuples::{Tuple2, Tuple3, Tuple4, Tuple5},
    unit::{unit, Unit},
    vec::BwsVec,
};
use std::future::Future;
use std::task::Poll;
pub use vtable::VTable;
use vtable::VTABLE;

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
pub struct SubPluginGate {
    receiver: *const (),
}
