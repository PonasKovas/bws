use std::fmt::Debug;

/// `#[repr(C)]` equivalent of `String`
#[repr(C)]
pub struct BwsString {
    pub(crate) ptr: *mut u8,
    pub(crate) length: usize,
    pub(crate) capacity: usize,
}

/// `#[repr(C)]` equivalent of `&'a str`
#[repr(C)]
pub struct BwsStr<'a> {
    pub(crate) ptr: &'a u8,
    pub(crate) length: usize,
}

impl BwsString {
    pub fn from_string(mut s: String) -> Self {
        let bws_string = Self {
            ptr: s.as_mut_str().as_mut_ptr(),
            length: s.len(),
            capacity: s.capacity(),
        };

        Box::leak(s.into_boxed_str());

        bws_string
    }
    pub fn into_string(self) -> String {
        unsafe { String::from_raw_parts(self.ptr, self.length, self.capacity) }
    }
    pub fn as_bws_str<'a>(&'a self) -> BwsStr<'a> {
        BwsStr {
            ptr: unsafe { self.ptr.as_ref().unwrap() },
            length: self.length,
        }
    }
}

impl<'a> BwsStr<'a> {
    pub fn from_str(s: &'a str) -> Self {
        Self {
            ptr: unsafe { s.as_bytes().as_ptr().as_ref().unwrap() },
            length: s.len(),
        }
    }
    pub fn as_str<'b>(&'b self) -> &'a str {
        unsafe { &std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.ptr, self.length)) }
    }
}

impl Debug for BwsString {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Debug::fmt(&self.as_bws_str(), f)
    }
}

impl<'a> Debug for BwsStr<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Debug::fmt(self.as_str(), f)
    }
}
