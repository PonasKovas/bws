use super::*;
use std::{
    fmt::{Debug, Display},
    hash::Hash,
    ops::Deref,
};

/// FFI-safe equivalent of `&str`
#[repr(C)]
#[derive(Copy, Clone)]
pub struct SStr<'a> {
    pub(crate) slice: SSlice<'a, u8>,
}

impl<'a> SStr<'a> {
    pub const fn from_str(s: &'a str) -> Self {
        Self {
            slice: SSlice::from_slice(s.as_bytes()),
        }
    }
    pub const fn into_str(self) -> &'a str {
        unsafe { std::str::from_utf8_unchecked(self.slice.into_slice()) }
    }
}

impl<'a> Deref for SStr<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.into_str()
    }
}

impl<'a> Display for SStr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self.into_str(), f)
    }
}

impl<'a> Debug for SStr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.into_str(), f)
    }
}

impl<'a> From<&'a str> for SStr<'a> {
    fn from(s: &'a str) -> Self {
        Self::from_str(s)
    }
}

impl<'a> From<&'a String> for SStr<'a> {
    fn from(s: &'a String) -> Self {
        Self::from_str(s.as_str())
    }
}

impl<'a> From<SStr<'a>> for &'a str {
    fn from(s: SStr<'a>) -> Self {
        s.into_str()
    }
}

impl<'a> PartialEq for SStr<'a> {
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(self.into_str(), other.into_str())
    }
}

impl<'a> Hash for SStr<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Hash::hash(self.into_str(), state)
    }
}

impl<'a> Eq for SStr<'a> {}
