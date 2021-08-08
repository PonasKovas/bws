use std::mem::transmute;

#[repr(C)]
pub struct BwsVec {
    ptr: *mut (),
    length: usize,
    capacity: usize,
}

impl BwsVec {
    pub fn from_vec<T: Sized>(mut v: Vec<T>) -> Self {
        let bws_vec = Self {
            ptr: v.as_mut_ptr() as *mut (),
            length: v.len(),
            capacity: v.capacity(),
        };

        v.leak();

        bws_vec
    }
    pub unsafe fn into_vec<T: Sized>(self) -> Vec<T> {
        Vec::from_raw_parts(transmute(self.ptr), self.length, self.capacity)
    }
}
