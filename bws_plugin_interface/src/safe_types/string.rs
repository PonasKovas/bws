use std::{
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
};

use super::*;

/// FFI-safe equivalent of `String`
#[repr(C)]
pub struct SString {
    inner: SVec<u8>,
}

impl SString {
    pub fn from_string(s: String) -> Self {
        Self {
            inner: SVec::from_vec(s.into_bytes()),
        }
    }
    pub fn into_string(self) -> String {
        unsafe { String::from_utf8_unchecked(self.inner.into_vec()) }
    }
    pub fn as_str<'a>(&'a self) -> &'a str {
        unsafe { std::str::from_utf8_unchecked(self.inner.as_slice()) }
    }
    pub fn as_str_mut<'a>(&'a mut self) -> &'a mut str {
        unsafe { std::str::from_utf8_unchecked_mut(self.inner.as_slice_mut()) }
    }
}

impl From<String> for SString {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}

impl From<SString> for String {
    fn from(s: SString) -> Self {
        s.into_string()
    }
}

impl Display for SString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.as_str(), f)
    }
}

impl Debug for SString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.as_str(), f)
    }
}

impl PartialEq for SString {
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(&self.as_str(), &other.as_str())
    }
}

impl Deref for SString {
    type Target = str;

    fn deref<'a>(&'a self) -> &'a Self::Target {
        self.as_str()
    }
}

impl DerefMut for SString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_str_mut()
    }
}
