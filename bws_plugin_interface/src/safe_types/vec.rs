use std::fmt::Debug;
use std::mem::forget;
use std::ops::{Deref, DerefMut, Index, IndexMut};

/// FFI-safe equivalent of `Vec<T>`
#[repr(C)]
pub struct SVec<T> {
    ptr: *mut T,
    length: usize,
    capacity: usize,
}

impl<T> SVec<T> {
    pub fn from_vec(mut v: Vec<T>) -> Self {
        let r = Self {
            ptr: v.as_mut_ptr(),
            length: v.len(),
            capacity: v.capacity(),
        };

        forget(v);

        r
    }
    pub fn into_vec(self) -> Vec<T> {
        let r = unsafe { Vec::from_raw_parts(self.ptr, self.length, self.capacity) };

        forget(self);

        r
    }
    pub fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.length) }
    }
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.length) }
    }
}

impl<T> Drop for SVec<T> {
    fn drop(&mut self) {
        let _: Vec<T> = unsafe { Vec::from_raw_parts(self.ptr, self.length, self.capacity) };
    }
}

impl<T> From<Vec<T>> for SVec<T> {
    fn from(v: Vec<T>) -> Self {
        Self::from_vec(v)
    }
}
impl<T> From<SVec<T>> for Vec<T> {
    fn from(v: SVec<T>) -> Self {
        v.into_vec()
    }
}

unsafe impl<T> Send for SVec<T> {}
unsafe impl<T> Sync for SVec<T> {}

impl<T: Debug> Debug for SVec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // deref to slice
        Debug::fmt(&**self, f)
    }
}

impl<T: Clone> Clone for SVec<T> {
    fn clone(&self) -> Self {
        self.to_vec().into()
    }
}

impl<T: PartialEq> PartialEq for SVec<T> {
    fn eq(&self, other: &Self) -> bool {
        // deref to slice
        PartialEq::eq(&**self, &**other)
    }
}

impl<T> Index<usize> for SVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        // deref to slice
        &(**self)[index]
    }
}

impl<T> IndexMut<usize> for SVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        // deref to slice
        &mut (**self)[index]
    }
}

impl<'a, T> IntoIterator for &'a SVec<T> {
    type Item = &'a T;

    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        // deref to slice
        (**self).iter()
    }
}

impl<T> Deref for SVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> DerefMut for SVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_slice_mut()
    }
}
