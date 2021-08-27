use crate::prelude::*;
use crate::vtable::VTABLE;
use async_ffi::ContextExt;
use std::future::Future;
use std::task::Poll;

/// A gate for the plugin side, newtype around a pointer to the receiver
/// Allows to handle events
#[repr(transparent)]
pub struct BwsPluginGate(*const ());

unsafe impl Send for BwsPluginGate {}

impl BwsPluginGate {
    pub unsafe fn new(receiver: *const ()) -> Self {
        Self(receiver)
    }
}

pub struct BwsPluginEventGuard {
    event: Option<BwsEvent<'static>>,
    sender: *const (),
}

impl BwsPluginEventGuard {
    /// Obtain the underlying [`PluginEvent`].
    ///
    /// ## Panics
    ///
    /// Panics if the method is called twice on the same [`PluginEventGuard`]
    pub fn event<'b>(&'b mut self) -> BwsEvent<'b> {
        unsafe {
            std::mem::transmute::<BwsEvent<'static>, BwsEvent<'b>>(
                self.event
                    .take()
                    .expect("Tried to call PluginEventGuard::event() twice"),
            )
        }
    }
    /// Finishes the event call.
    pub fn finish(self) {
        unsafe {
            (VTABLE.send_oneshot)(self.sender);
        }
    }
}

impl Future for &mut BwsPluginGate {
    type Output = Option<BwsPluginEventGuard>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        ctx: &mut std::task::Context,
    ) -> std::task::Poll<Self::Output> {
        match unsafe { ctx.with_ffi_context(|ctx| (VTABLE.recv_plugin_event)(self.0, ctx)) }
            .try_into_poll()
            .unwrap_or_else(|_| panic!("FFI future for receiving event panicked."))
        {
            Poll::Ready(r) => Poll::Ready(r.into_option().map(|BwsTuple2(event, oneshot_ptr)| {
                BwsPluginEventGuard {
                    event: Some(event),
                    sender: oneshot_ptr,
                }
            })),
            Poll::Pending => Poll::Pending,
        }
    }
}
