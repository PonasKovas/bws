use std::fmt::Debug;

/// `#[repr(C)]` equivalent of `Vec<T>`
#[repr(C)]
pub struct BwsVec<T: Sized> {
    pub(crate) ptr: *mut T,
    pub(crate) length: usize,
    pub(crate) capacity: usize,
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
    pub fn into_vec(self) -> Vec<T> {
        unsafe { Vec::from_raw_parts(self.ptr, self.length, self.capacity) }
    }
    pub fn as_bws_slice<'a>(&'a self) -> super::slice::BwsSlice<'a, T> {
        super::slice::BwsSlice {
            ptr: unsafe { self.ptr.as_ref().unwrap() },
            length: self.length,
        }
    }
}

impl<T: Sized + Debug> Debug for BwsVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Debug::fmt(&self.as_bws_slice(), f)
    }
}
