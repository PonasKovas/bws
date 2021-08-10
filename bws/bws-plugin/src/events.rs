use crate::BwsStr;
use variant_count::VariantCount;

#[derive(VariantCount, Debug)]
#[repr(C, u32)]
pub enum PluginEvent {
    Arbitrary(BwsStr, *const ()),
}

#[derive(VariantCount)]
#[repr(C, u32)]
pub enum SubPluginEvent {
    Arbitrary(BwsStr, *const ()),
}
