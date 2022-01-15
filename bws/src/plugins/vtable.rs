//! the VTable that is given to the plugins so they can do stuff 😐

use async_ffi::{FfiContext, FfiFuture, FfiPoll};
use bws_plugin::vtable::BwsVTable;
use bws_plugin::{prelude::*, LogLevel};
use log::{debug, error, info, trace, warn};
use std::mem::transmute;
use tokio::sync::mpsc;
use tokio::sync::oneshot::Sender;

pub static VTABLE: BwsVTable = {
    unsafe extern "C" fn poll_recv_plugin_event(
        receiver: SendPtr<()>,
        ctx: &mut FfiContext,
    ) -> FfiPoll<BwsOption<BwsTuple3<u32, SendPtr<()>, SendPtr<()>>>> {
        let receiver: &mut mpsc::UnboundedReceiver<BwsTuple3<u32, SendPtr<()>, SendPtr<()>>> =
            unsafe { transmute(receiver) };
        match ctx.with_context(|ctx| receiver.poll_recv(ctx)) {
            std::task::Poll::Ready(r) => FfiPoll::Ready(BwsOption::from_option(r)),
            std::task::Poll::Pending => FfiPoll::Pending,
        }
    }

    unsafe extern "C" fn fire_oneshot_plugin_event(sender: SendPtr<()>) -> bool {
        let sender: *const Sender<()> = unsafe { transmute(sender) };

        unsafe { sender.read() }.send(()).is_ok()
    }

    unsafe extern "C" fn log(plugin_name: BwsStr<'static>, msg: BwsStr<'static>, level: LogLevel) {
        log::log!(
            target: &format!("[plugin] {}", plugin_name.as_str()),
            match level {
                LogLevel::Error => {
                    log::Level::Error
                }
                LogLevel::Warning => {
                    log::Level::Warn
                }
                LogLevel::Info => {
                    log::Level::Info
                }
                LogLevel::Debug => {
                    log::Level::Debug
                }
                LogLevel::Trace => {
                    log::Level::Trace
                }
            },
            "{}",
            msg.as_str()
        );
    }

    unsafe extern "C" fn spawn_task(future: FfiFuture<BwsUnit>) {
        tokio::spawn(future);
    }

    unsafe extern "C" fn get_port() -> u16 {
        crate::OPT.port
    }

    unsafe extern "C" fn get_compression_treshold() -> i32 {
        crate::OPT.compression_treshold
    }

    BwsVTable {
        poll_recv_plugin_event,
        fire_oneshot_plugin_event,
        log,
        spawn_task,
        get_port,
        get_compression_treshold,
    }
};
