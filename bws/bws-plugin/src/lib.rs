mod events;
mod stable_types;

use std::future::Future;

pub use events::{PluginEvent, SubPluginEvent};
pub use stable_types::{
    option::BwsOption,
    slice::BwsSlice,
    string::{BwsStr, BwsString},
    tuples::{Tuple2, Tuple3, Tuple4, Tuple5},
    unit::{unit, Unit},
    vec::BwsVec,
};

use async_ffi::{FfiContext, FfiFuture, FfiPoll};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PluginEventReceiver(pub &'static ());

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SubPluginEventReceiver(pub &'static ());

#[repr(C)]
#[derive(Copy, Clone)]
pub struct OneshotSender(pub &'static ());

// FFI function signatures
pub type RegisterPlugin = unsafe extern "C" fn(
    BwsStr,
    Tuple3<u64, u64, u64>,
    BwsSlice<Tuple2<BwsStr, BwsStr>>,
) -> Tuple5<
    RecvPluginEvent,
    PluginEventReceiver,
    PluginEntry,
    PluginSubscribeToEvent,
    RegisterSubPlugin,
>;
pub type PluginEntry = unsafe extern "C" fn(FfiFuture<Unit>);
pub type RecvPluginEvent =
    unsafe extern "C" fn(
        PluginEventReceiver,
        &mut FfiContext,
    ) -> FfiPoll<BwsOption<Tuple2<PluginEvent, OneshotSender>>>;
pub type PluginSubscribeToEvent = unsafe extern "C" fn(BwsStr);
pub type RegisterSubPlugin = unsafe extern "C" fn(
    BwsStr,
) -> Tuple3<
    SubPluginEventReceiver,
    SubPluginEntry,
    SubPluginSubscribeToEvent,
>;
pub type SubPluginEntry = unsafe extern "C" fn(FfiFuture<Unit>);
pub type RecvSubPluginEvent =
    unsafe extern "C" fn(
        &mut (),
        &mut FfiContext,
    ) -> FfiPoll<BwsOption<Tuple2<SubPluginEvent, OneshotSender>>>;
pub type SubPluginSubscribeToEvent = unsafe extern "C" fn(BwsStr);
