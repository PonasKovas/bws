#![allow(unused_imports)]

pub mod global_state;

use abi_stable::{
    sabi_types::{RMut, VersionStrings},
    std_types::{RArc, RBox, RBoxError, RCow, RResult, RSlice, RStr, RString, RVec, Tuple2},
};

pub const ABI: u32 = 14;

#[derive(Debug)]
#[repr(C)]
pub struct BwsPlugin {
    pub name: RStr<'static>,
    pub version: RStr<'static>,
    pub dependencies: RSlice<'static, Tuple2<RStr<'static>, RStr<'static>>>,

    pub enable: fn(),
    pub disable: fn(),
}
