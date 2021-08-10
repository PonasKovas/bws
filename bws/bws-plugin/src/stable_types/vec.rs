#[repr(C)]
pub struct BwsVec<T: Sized> {
    ptr: *mut T,
    length: usize,
    capacity: usize,
}

impl<T: Sized> BwsVec<T> {
    pub fn from_vec(mut v: Vec<T>) -> Self {
        let bws_vec = Self {
            ptr: v.as_mut_ptr(),
            length: v.len(),
            capacity: v.capacity(),
        };

        v.leak();

        bws_vec
    }
    pub unsafe fn into_vec(self) -> Vec<T> {
        Vec::from_raw_parts(self.ptr, self.length, self.capacity)
    }
}
