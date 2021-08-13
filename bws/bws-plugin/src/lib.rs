#![allow(non_camel_case_types)]

pub mod events;
pub mod pointers;
pub mod stable_types;
pub mod vtable;

use async_ffi::{ContextExt, FfiContext, FfiFuture, FfiPoll};
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
use std::future::Future;
use std::task::Poll;

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
    receiver: *const (),
    receive: _f_RecvPluginEvent,
    send: _f_SendOneshot,
}

unsafe impl Send for PluginGate {}

impl PluginGate {
    pub unsafe fn new(
        receiver: *const (),
        receive: _f_RecvPluginEvent,
        send: _f_SendOneshot,
    ) -> Self {
        Self {
            receiver,
            receive,
            send,
        }
    }
}

pub struct PluginEventGuard {
    event: Option<PluginEvent<'static>>,
    sender: *const (),
}

impl PluginEventGuard {
    /// Obtain the underlying [`PluginEvent`].
    ///
    /// ## Panics
    ///
    /// Panics if the method is called twice on the same [`PluginEventGuard`]
    pub fn event<'a>(&'a mut self) -> PluginEvent<'a> {
        unsafe {
            std::mem::transmute::<PluginEvent<'static>, PluginEvent<'a>>(
                self.event
                    .take()
                    .expect("Tried to call PluginEventGuard::event() twice"),
            )
        }
    }
    /// Finishes the event call.
    pub fn finish(self) {
        println!("{:?}", self.sender);
    }
}

impl Future for &mut PluginGate {
    type Output = Option<PluginEventGuard>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        ctx: &mut std::task::Context,
    ) -> std::task::Poll<Self::Output> {
        match unsafe { ctx.with_ffi_context(|ctx| (self.receive)(self.receiver, ctx)) }
            .try_into_poll()
            .unwrap_or_else(|_| panic!("FFI future for receiving event panicked."))
        {
            Poll::Ready(r) => {
                Poll::Ready(
                    r.into_option()
                        .map(|Tuple2(event, oneshot_ptr)| PluginEventGuard {
                            event: Some(event),
                            sender: oneshot_ptr,
                        }),
                )
            }
            Poll::Pending => Poll::Pending,
        }
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
        *const (),
        &mut FfiContext,
    ) -> FfiPoll<BwsOption<Tuple2<PluginEvent<'static>, *const ()>>>;
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
