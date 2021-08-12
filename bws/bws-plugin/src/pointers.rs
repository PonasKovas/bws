use std::ops::Deref;

/// Newtype wrapper of a pointer to an unstable `tokio::sync::mpsc::UnboundedReceiver<Tuple2<PluginEvent, BwsOneshotSender>>`
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct BwsPluginEventReceiver(&'static ());

/// Newtype wrapper of a pointer to an unstable `tokio::sync::mpsc::UnboundedReceiver<Tuple2<SubPluginEvent, BwsOneshotSender>>`
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct BwsSubPluginEventReceiver(&'static ());

/// Newtype wrapper of a pointer to an unstable `tokio::sync::oneshot::Sender<*const ()>`
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct BwsOneshotSender(&'static ());

macro_rules! impl_ptr {
    ($($typename:ident),*) => {
        $(
            impl Deref for $typename {
                type Target = &'static ();

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl $typename {
                pub unsafe fn new(reference: &'static ()) -> Self {
                    Self(reference)
                }
            }
        )*

    };
}

impl_ptr!(
    BwsPluginEventReceiver,
    BwsSubPluginEventReceiver,
    BwsOneshotSender
);
