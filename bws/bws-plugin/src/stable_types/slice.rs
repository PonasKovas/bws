use std::fmt::Debug;

/// `#[repr(C)]` equivalent of `&'a [T]`
#[repr(C)]
pub struct BwsSlice<'a, T: Sized> {
    pub(crate) ptr: &'a T,
    pub(crate) length: usize,
}

impl<'a, T: Sized> BwsSlice<'a, T> {
    pub fn from_slice(s: &[T]) -> Self {
        Self {
            ptr: unsafe { s.as_ptr().as_ref().unwrap() },
            length: s.len(),
        }
    }
    pub fn as_slice<'b>(&'b self) -> &'a [T] {
        unsafe { &std::slice::from_raw_parts(self.ptr as *const _, self.length) }
    }
}

impl<'a, T: Sized + Debug> Debug for BwsSlice<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Debug::fmt(self.as_slice(), f)
    }
}
