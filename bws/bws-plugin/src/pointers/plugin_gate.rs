use crate::*;

/// A gate for the plugin side, newtype around a pointer to the receiver
/// Allows to handle events
#[repr(transparent)]
pub struct PluginGate(*const ());

unsafe impl Send for PluginGate {}

impl PluginGate {
    pub unsafe fn new(receiver: *const ()) -> Self {
        Self(receiver)
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
        unsafe {
            (VTABLE.send_oneshot)(self.sender);
        }
    }
}

impl Future for &mut PluginGate {
    type Output = Option<PluginEventGuard>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        ctx: &mut std::task::Context,
    ) -> std::task::Poll<Self::Output> {
        match unsafe { ctx.with_ffi_context(|ctx| (VTABLE.recv_plugin_event)(self.0, ctx)) }
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
