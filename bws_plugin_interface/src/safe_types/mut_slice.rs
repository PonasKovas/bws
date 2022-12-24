use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut, Index, IndexMut},
};

/// FFI-safe equivalent of `&mut [T]`
///
/// See documentation of [`slice`]
#[repr(C)]
pub struct SMutSlice<'a, T> {
    ptr: *mut T,
    length: usize,
    _phantom_d: PhantomData<&'a T>,
}

impl<'a, T> SMutSlice<'a, T> {
    pub fn from_slice(slice: &'a mut [T]) -> Self {
        Self {
            ptr: slice.as_mut_ptr(),
            length: slice.len(),
            _phantom_d: PhantomData,
        }
    }
    pub fn into_slice(self) -> &'a mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.length) }
    }
    pub const fn as_slice<'b>(&'b self) -> &'b [T]
    where
        'a: 'b,
    {
        unsafe { std::slice::from_raw_parts(self.ptr, self.length) }
    }
    pub fn as_slice_mut<'b>(&'b mut self) -> &'b mut [T]
    where
        'a: 'b,
    {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.length) }
    }
}

impl<'a, T> Deref for SMutSlice<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a, T> DerefMut for SMutSlice<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_slice_mut()
    }
}

impl<'a, T> From<&'a mut [T]> for SMutSlice<'a, T> {
    fn from(s: &'a mut [T]) -> Self {
        Self::from_slice(s)
    }
}

impl<'a, T> From<SMutSlice<'a, T>> for &'a [T] {
    fn from(s: SMutSlice<'a, T>) -> Self {
        s.into_slice()
    }
}
impl<'a, T> From<SMutSlice<'a, T>> for &'a mut [T] {
    fn from(s: SMutSlice<'a, T>) -> Self {
        s.into_slice()
    }
}

impl<'a, T, const N: usize> From<&'a mut [T; N]> for SMutSlice<'a, T> {
    fn from(s: &'a mut [T; N]) -> Self {
        Self::from_slice(s.as_mut_slice())
    }
}

impl<'a, T, I> Index<I> for SMutSlice<'a, T>
where
    I: std::slice::SliceIndex<[T]>,
{
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &I::Output {
        self.as_slice().index(index)
    }
}

impl<'a, T, I> IndexMut<I> for SMutSlice<'a, T>
where
    I: std::slice::SliceIndex<[T]>,
{
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut I::Output {
        self.as_slice_mut().index_mut(index)
    }
}

impl<'a, T> IntoIterator for SMutSlice<'a, T> {
    type Item = &'a mut T;

    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_slice().into_iter()
    }
}

unsafe impl<'a, T> Send for SMutSlice<'a, T> {}
unsafe impl<'a, T> Sync for SMutSlice<'a, T> {}
