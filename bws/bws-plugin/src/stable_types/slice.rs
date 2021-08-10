use std::mem::transmute;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct BwsSlice<T: Sized> {
    ptr: *const T,
    length: usize,
}

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
