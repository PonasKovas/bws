//! the VTable that is given to the plugins so they can do stuff 😐

use async_ffi::{FfiContext, FfiPoll};
use bws_plugin::prelude::*;
use bws_plugin::vtable::BwsVTable;
use std::mem::transmute;
use tokio::sync::mpsc;

pub static VTABLE: BwsVTable = {
    unsafe extern "C" fn poll_recv_plugin_event(
        receiver: *const (),
        ctx: &mut FfiContext,
    ) -> FfiPoll<BwsOption<BwsTuple3<u32, *const (), *const ()>>> {
        let receiver: &mut mpsc::UnboundedReceiver<BwsTuple3<u32, *const (), *const ()>> =
            transmute(receiver);
        match ctx.with_context(|ctx| receiver.poll_recv(ctx)) {
            std::task::Poll::Ready(r) => FfiPoll::Ready(BwsOption::from_option(r)),
            std::task::Poll::Pending => FfiPoll::Pending,
        }
    }

    BwsVTable {
        poll_recv_plugin_event,
    }
};
