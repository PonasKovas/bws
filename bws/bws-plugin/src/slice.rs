use std::mem::transmute;

#[repr(C)]
pub struct BwsSlice {
    ptr: *const (),
    length: usize,
}

impl BwsSlice {
    pub fn from_slice<T: Sized>(s: &[T]) -> Self {
        Self {
            ptr: s.as_ptr() as *const (),
            length: s.len(),
        }
    }
    pub unsafe fn into_slice<'a, T: Sized>(self) -> &'a [T] {
        &std::slice::from_raw_parts(transmute(self.ptr), self.length)
    }
}
