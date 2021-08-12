#![allow(non_camel_case_types)]

mod events;
mod pointers;
mod stable_types;

use std::future::Future;

pub use events::{PluginEvent, SubPluginEvent};
pub use pointers::{BwsOneshotSender, BwsPluginEventReceiver, BwsSubPluginEventReceiver};
pub use stable_types::{
    global_state::BwsGlobalState,
    option::BwsOption,
    slice::BwsSlice,
    string::{BwsStr, BwsString},
    tuples::{Tuple2, Tuple3, Tuple4, Tuple5},
    unit::{unit, Unit},
    vec::BwsVec,
};

use async_ffi::{ContextExt, FfiContext, FfiFuture, FfiPoll};

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

/// A gate for the plugin side, bundles all that is required to handle events
#[repr(C)]
pub struct PluginGate {
    receiver: BwsPluginEventReceiver,
    receive: _f_RecvPluginEvent,
    send: _f_SendOneshot,
}

impl PluginGate {
    pub unsafe fn new(
        receiver: BwsPluginEventReceiver,
        receive: _f_RecvPluginEvent,
        send: _f_SendOneshot,
    ) -> Self {
        Self {
            receiver,
            receive,
            send,
        }
    }
    pub fn finish(&mut self, sender: BwsOneshotSender) {
        unsafe {
            (self.send)(sender);
        }
    }
}

impl Future for &mut PluginGate {
    type Output = BwsOption<Tuple2<PluginEvent, BwsOneshotSender>>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        ctx: &mut std::task::Context,
    ) -> std::task::Poll<Self::Output> {
        unsafe { ctx.with_as_ffi_context(|ctx| (self.receive)(self.receiver, ctx)) }.into_poll()
    }
}

/// A gate for the plugin side, bundles all that is required to handle events for sub-plugins
#[repr(C)]
pub struct SubPluginGate {
    receiver: BwsSubPluginEventReceiver,
    receive: _f_RecvSubPluginEvent,
    send: _f_SendOneshot,
}

/////////////////////////////
// FFI function signatures //
/////////////////////////////

/// Defined on BWS, a poll fn that plugins can wrap in a [`Future`][std::future::Future] to
/// receive events from the plugin `Gate`.
pub type _f_RecvPluginEvent =
    unsafe extern "C" fn(
        BwsPluginEventReceiver,
        &mut FfiContext,
    ) -> FfiPoll<BwsOption<Tuple2<PluginEvent, BwsOneshotSender>>>;
/// Defined on BWS, a poll fn that sub-plugins can wrap in a [`Future`][std::future::Future] to
/// receive events from the sub-plugin `Gate`.
pub type _f_RecvSubPluginEvent =
    unsafe extern "C" fn(
        BwsSubPluginEventReceiver,
        &mut FfiContext,
    ) -> FfiPoll<BwsOption<Tuple2<SubPluginEvent, BwsOneshotSender>>>;
/// Defined on BWS, lets plugins send data to the tokio `Oneshot` channels.
///
/// This is used to finish an event call.
pub type _f_SendOneshot = unsafe extern "C" fn(BwsOneshotSender);

/// Defined on BWS, lets dynamic libraries register a plugin.
///
/// ## Arguments
///
/// 1. Name of the plugin (should be unique).
/// 2. Version of the plugin (`major`, `minor`, `patch`) in SemVer format.
/// 3. A list of dependencies, (name of the dependency, version requirement)
/// 4. A function pointer to the plugin entry.
///
/// ## Returned values
///
/// Returns a tuple:
/// 1. [`_f_PluginSubscribeToEvent`] fn for the plugin to subscribe to certain events.
/// 1. [`_f_RegisterSubPlugin`] fn for the plugin to register subplugins.
pub type _f_RegisterPlugin =
    unsafe extern "C" fn(
        BwsStr,
        Tuple3<u64, u64, u64>,
        BwsSlice<Tuple2<BwsStr, BwsStr>>,
        _f_PluginEntry,
    ) -> Tuple2<_f_PluginSubscribeToEvent, _f_RegisterSubPlugin>;
/// Defined on the plugin, starts the plugin. Gives the name of the plugin and the gate.
pub type _f_PluginEntry =
    unsafe extern "C" fn(BwsStr, PluginGate, BwsGlobalState) -> FfiFuture<Unit>;

/// Defined on BWS, lets plugins subscribe to events during (AND ONLY DURING) plugin initialization.
pub type _f_PluginSubscribeToEvent = unsafe extern "C" fn(BwsStr);

/// Defined on BWS, lets plugins register subplugins
///
/// Takes the name of the subplugin and the entry function pointer
pub type _f_RegisterSubPlugin =
    unsafe extern "C" fn(BwsStr, _f_SubPluginEntry) -> _f_SubPluginSubscribeToEvent;

pub type _f_SubPluginEntry = unsafe extern "C" fn(BwsStr, SubPluginGate) -> FfiFuture<Unit>;
pub type _f_SubPluginSubscribeToEvent = unsafe extern "C" fn(BwsStr);
