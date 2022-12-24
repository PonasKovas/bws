use super::*;
use std::{
    fmt::{Debug, Display},
    hash::Hash,
    ops::{Deref, DerefMut},
};

/// FFI-safe equivalent of `&str`
#[derive(Copy, Clone)]
#[repr(C)]
pub struct SStr<'a> {
    slice: SSlice<'a, u8>,
}

impl<'a> SStr<'a> {
    pub const fn from_str(s: &'a str) -> Self {
        Self {
            slice: SSlice::from_slice(s.as_bytes()),
        }
    }
    pub const fn as_str<'b>(&'b self) -> &'b str
    where
        'a: 'b,
    {
        unsafe { &std::str::from_utf8_unchecked(self.slice.as_slice()) }
    }
    pub const fn into_str(self) -> &'a str {
        unsafe { std::str::from_utf8_unchecked(self.slice.into_slice()) }
    }
}

impl<'a> Deref for SStr<'a> {
    type Target = str;

    fn deref<'b>(&'b self) -> &'b Self::Target {
        self.as_str()
    }
}

impl<'a> Display for SStr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.as_str(), f)
    }
}

impl<'a> Debug for SStr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}

impl<'a> From<&'a str> for SStr<'a> {
    fn from(s: &'a str) -> Self {
        Self::from_str(s)
    }
}

impl<'a> From<SStr<'a>> for &'a str {
    fn from(s: SStr<'a>) -> Self {
        s.into_str()
    }
}

impl<'a> PartialEq for SStr<'a> {
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(self.as_str(), other.as_str())
    }
}

impl<'a> Hash for SStr<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Hash::hash(self.as_str(), state)
    }
}

impl<'a> Eq for SStr<'a> {}
