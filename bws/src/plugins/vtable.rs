use super::{CandidatePlugin, SubPluginData};
use crate::global_state::{GlobalState, InnerGlobalState};
use async_ffi::FfiContext;
use async_ffi::FfiPoll;
use bitvec::prelude::*;
use bws_plugin::{
    prelude::*,
    register::{_f_PluginEntry, _f_SubPluginEntry},
    vtable::BwsVTable,
};
use log::{error, info, warn};
use semver::Version;
use std::collections::HashSet;
use std::mem::transmute;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

pub(super) static mut PLUGINS_TO_REGISTER: Vec<CandidatePlugin> = Vec::new();

pub static VTABLE: BwsVTable = {
    unsafe extern "C" fn register_plugin(
        name: BwsStr,
        version: BwsTuple3<u64, u64, u64>,
        dependencies: BwsSlice<BwsTuple2<BwsStr, BwsStr>>,
        events: BwsSlice<u32>,
        entry: _f_PluginEntry,
    ) -> usize {
        let mut plugin = CandidatePlugin {
            name: name.as_str().to_owned(),
            version: Version::new(version.0, version.1, version.2),
            dependencies: dependencies
                .as_slice()
                .iter()
                .map(|e| (e.0.as_str().to_owned(), e.1.as_str().to_owned()))
                .collect(),
            subscribed_events: BitVec::new(),
            entry,
            subplugins: Vec::new(),
        };

        for event in events.as_slice() {
            place(&mut plugin.subscribed_events, *event as usize, true);
        }

        PLUGINS_TO_REGISTER.push(plugin);

        PLUGINS_TO_REGISTER.len() - 1
    }
    unsafe extern "C" fn register_subplugin(
        plugin: usize,
        name: BwsStr,
        events: BwsSlice<u32>,
        entry: _f_SubPluginEntry,
    ) {
        let plugin = &mut PLUGINS_TO_REGISTER[plugin];

        let mut subplugin = SubPluginData {
            name: name.as_str().to_owned(),
            subscribed_events: BitVec::new(),
            entry,
        };

        for event in events.as_slice() {
            place(&mut subplugin.subscribed_events, *event as usize, true);
        }

        plugin.subplugins.push(subplugin);
    }
    unsafe extern "C" fn recv_plugin_event(
        receiver: *const (),
        ctx: &mut FfiContext,
    ) -> FfiPoll<BwsOption<BwsTuple2<BwsEvent<'static>, *const ()>>> {
        let receiver: &mut mpsc::UnboundedReceiver<BwsTuple2<BwsEvent, *const ()>> =
            transmute(receiver);
        match ctx.with_context(|ctx| receiver.poll_recv(ctx)) {
            std::task::Poll::Ready(r) => FfiPoll::Ready(BwsOption::from_option(r)),
            std::task::Poll::Pending => FfiPoll::Pending,
        }
    }
    unsafe extern "C" fn send_oneshot(sender: *const ()) {
        let sender = std::ptr::read(sender as *const oneshot::Sender<()>);
        if sender.send(()).is_err() {
            error!("Error completing event call.");
        }
    }
    unsafe extern "C" fn drop_global_state(arc: *const ()) {
        Arc::decrement_strong_count(arc as *const InnerGlobalState)
    }
    unsafe extern "C" fn gs_get_compression_treshold(gs: *const ()) -> i32 {
        (*(gs as *const InnerGlobalState)).compression_treshold
    }
    unsafe extern "C" fn gs_get_port(gs: *const ()) -> u16 {
        (*(gs as *const InnerGlobalState)).port
    }

    BwsVTable {
        register_plugin,
        register_subplugin,
        recv_plugin_event,
        send_oneshot,
        drop_global_state,
        gs_get_compression_treshold,
        gs_get_port,
    }
};

/// Places a value to a specific position in a bitvec,
/// increasing the size of the vec if necessary.
fn place(bits: &mut BitVec, pos: usize, val: bool) {
    if bits.len() >= pos {
        bits.resize(pos + 1, false);
    }
    bits.set(pos, val);
}
