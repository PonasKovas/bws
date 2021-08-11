use std::{fmt::Debug, mem::transmute};

#[repr(C)]
pub struct BwsSlice<T: Sized> {
    ptr: *const T,
    length: usize,
}

impl<T: Sized> Clone for BwsSlice<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            length: self.length,
        }
    }
}
impl<T: Sized> Copy for BwsSlice<T> {}

impl<T: Sized> BwsSlice<T> {
    pub fn from_slice(s: &[T]) -> Self {
        Self {
            ptr: s.as_ptr(),
            length: s.len(),
        }
    }
    pub unsafe fn into_slice<'a>(self) -> &'a [T] {
        &std::slice::from_raw_parts(transmute(self.ptr), self.length)
    }
}

impl<T: Sized + Debug> Debug for BwsSlice<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        unsafe { Debug::fmt(self.into_slice(), f) }
    }
}
