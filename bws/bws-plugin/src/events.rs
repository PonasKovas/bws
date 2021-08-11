use crate::BwsStr;
use variant_count::VariantCount;

#[derive(VariantCount, Debug)]
#[repr(C, u32)]
pub enum PluginEvent {
    /// The [`BwsStr`] is not actually `'static`, it is valid until the event call is finished
    /// by sending a response to the oneshot channel.
    Arbitrary(BwsStr<'static>, *mut ()),
}

#[derive(VariantCount)]
#[repr(C, u32)]
pub enum SubPluginEvent {
    /// The [`BwsStr`] is not actually `'static`, it is valid until the event call is finished
    /// by sending a response to the oneshot channel.
    Arbitrary(BwsStr<'static>, *mut ()),
}
