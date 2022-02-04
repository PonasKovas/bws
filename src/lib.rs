#![allow(unused_imports)]

use abi_stable::{
    sabi_types::{RMut, VersionStrings},
    std_types::{RArc, RBox, RBoxError, RCow, RResult, RStr, RString},
};

pub const ABI: u32 = 13;

#[repr(C)]
pub struct BwsPlugin {
    pub name: RStr<'static>,
    pub version: RStr<'static>,
}
