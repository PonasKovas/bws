use crate::{BwsStr, SendMutPtr};
use variant_count::VariantCount;

#[derive(VariantCount, Debug)]
#[repr(C, u32)]
pub enum PluginEvent<'a> {
    /// The [`BwsStr`] is not actually `'static`, it is valid until the event call is finished
    /// by sending a response to the oneshot channel.
    // TODO make the finishing method take this to guarrantee there are no more references to it
    // once the call is finished?
    Arbitrary(BwsStr<'a>, SendMutPtr<()>),
}

#[derive(VariantCount, Debug)]
#[repr(C, u32)]
pub enum SubPluginEvent {
    /// The [`BwsStr`] is not actually `'static`, it is valid until the event call is finished
    /// by sending a response to the oneshot channel.
    Arbitrary(BwsStr<'static>, SendMutPtr<()>),
}
