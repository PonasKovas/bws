#![allow(non_camel_case_types)]

mod events;
mod stable_types;

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

/// Newtype wrapper of a pointer to an unstable `tokio::sync::mpsc::UnboundedReceiver<Tuple2<PluginEvent, BwsOneshotSender>>`
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct BwsPluginEventReceiver(pub &'static ());

/// Newtype wrapper of a pointer to an unstable `tokio::sync::mpsc::UnboundedReceiver<Tuple2<SubPluginEvent, BwsOneshotSender>>`
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct BwsSubPluginEventReceiver(pub &'static ());

/// Newtype wrapper of a pointer to an unstable `tokio::sync::oneshot::Sender<*const ()>`
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct BwsOneshotSender(pub &'static ());

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
/// Defined on BWS, lets plugins send data to the tokio `Oneshot` channels.
///
/// Usually this is used to finish an event call.
pub type _f_SendOneshot = unsafe extern "C" fn(BwsOneshotSender, *const ());

/// Defined on BWS, lets dynamic libraries register a plugin.
///
/// ## Arguments
///
/// 1. Name of the plugin (should be unique).
/// 2. Version of the plugin (`major`, `minor`, `patch`) in SemVer format.
/// 3. A list of dependencies, (name of the dependency, version requirement)
///
/// ## Returned values
///
/// Returns a tuple:
/// 1. [`_f_RecvPluginEvent`] poll fn for receiving plugin events
/// 2. [`_f_SendOneshot`] fn for sending data through a [`BwsOneshotSender`]
/// 3. [`BwsPluginEventReceiver`] a pointer to a leaked unstable `tokio::sync::mpsc::UnboundedReceiver`
///    that will be passed to the [`_f_RecvPluginEvent`].
/// 4. [`_f_PluginEntry`] fn to register the plugin's entry point with a future.
/// 5. [`_f_PluginSubscribeToEvent`] fn for the plugin to subscribe to certain events.
pub type _f_RegisterPlugin = unsafe extern "C" fn(
    BwsStr,
    Tuple3<u64, u64, u64>,
    BwsSlice<Tuple2<BwsStr, BwsStr>>,
) -> Tuple5<
    _f_RecvPluginEvent,
    _f_SendOneshot,
    BwsPluginEventReceiver,
    _f_PluginEntry,
    _f_PluginSubscribeToEvent,
>;
/// Defined on BWS, given a future will spawn it in a tokio task once the plugin is to be started.
pub type _f_PluginEntry = unsafe extern "C" fn(FfiFuture<Unit>);
/// Defined on BWS, lets plugins subscribe to events during (AND ONLY DURING) plugin initialization.
pub type _f_PluginSubscribeToEvent = unsafe extern "C" fn(BwsStr);
