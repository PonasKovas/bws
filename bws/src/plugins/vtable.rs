//! the VTable that is given to the plugins so they can do stuff 😐

use async_ffi::{FfiContext, FfiFuture, FfiPoll, FutureExt};
use bws_plugin::vtable::BwsVTable;
use bws_plugin::{prelude::*, LogLevel};
use log::{debug, error, info, trace, warn};
use std::mem::transmute;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc;
use tokio::sync::oneshot::Sender;

pub static VTABLE: BwsVTable = {
    unsafe extern "C" fn poll_recv_plugin_event(
        receiver: SendPtr<()>,
        ctx: &mut FfiContext,
    ) -> FfiPoll<BwsOption<BwsTuple3<usize, SendMutPtr<()>, SendPtr<()>>>> {
        let receiver: &mut mpsc::UnboundedReceiver<BwsTuple3<usize, SendMutPtr<()>, SendPtr<()>>> =
            unsafe { transmute(receiver) };
        match ctx.with_context(|ctx| receiver.poll_recv(ctx)) {
            std::task::Poll::Ready(r) => FfiPoll::Ready(BwsOption::from_option(r)),
            std::task::Poll::Pending => FfiPoll::Pending,
        }
    }

    unsafe extern "C" fn fire_oneshot_plugin_event(sender: SendPtr<()>, stop: bool) {
        let sender: *const Sender<bool> = unsafe { transmute(sender) };

        unsafe { sender.read() }.send(stop).unwrap()
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

    unsafe extern "C" fn shutdown() {
        crate::shutdown();
    }

    unsafe extern "C" fn register_for_graceful_shutdown(
    ) -> BwsTuple2<FfiFuture<BwsUnit>, SendPtr<()>> {
        let (mut receiver, atomic) = crate::register_for_graceful_shutdown();
        BwsTuple2(
            async move {
                let _ = receiver.recv().await;
                unit()
            }
            .into_ffi(),
            SendPtr(atomic as *const _ as *const ()),
        )
    }

    unsafe extern "C" fn gracefully_exited(atomic: SendPtr<()>) {
        unsafe { (atomic.0 as *const AtomicU32).as_ref() }
            .unwrap()
            .fetch_add(1, Ordering::SeqCst);
    }

    unsafe extern "C" fn get_event_id(
        namespace: BwsStr<'static>,
        name: BwsStr<'static>,
    ) -> FfiFuture<usize> {
        async move { super::events::get_event_id(namespace.as_str(), name.as_str()).await }
            .into_ffi()
    }

    BwsVTable {
        poll_recv_plugin_event,
        fire_oneshot_plugin_event,
        log,
        spawn_task,
        get_port,
        get_compression_treshold,
        shutdown,
        register_for_graceful_shutdown,
        gracefully_exited,
        get_event_id,
    }
};
