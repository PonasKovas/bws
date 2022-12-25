use std::{
    fmt::Debug,
    marker::PhantomData,
    ops::{Deref, Index},
};

/// FFI-safe equivalent of `&[T]`
///
/// See documentation of [`slice`]
#[repr(C)]
pub struct SSlice<'a, T> {
    ptr: *const T,
    length: usize,
    _phantom_d: PhantomData<&'a T>,
}

impl<'a, T> SSlice<'a, T> {
    pub const fn from_slice(slice: &[T]) -> Self {
        Self {
            ptr: slice.as_ptr(),
            length: slice.len(),
            _phantom_d: PhantomData,
        }
    }
    pub const fn into_slice(self) -> &'a [T] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.length) }
    }
    pub const fn as_slice<'b>(&'b self) -> &'b [T]
    where
        'a: 'b,
    {
        unsafe { std::slice::from_raw_parts(self.ptr, self.length) }
    }
}

impl<'a, T: Debug> Debug for SSlice<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.as_slice(), f)
    }
}

impl<'a, T> Deref for SSlice<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a, T> From<&'a [T]> for SSlice<'a, T> {
    fn from(s: &'a [T]) -> Self {
        Self::from_slice(s)
    }
}
impl<'a, T> From<&'a mut [T]> for SSlice<'a, T> {
    fn from(s: &'a mut [T]) -> Self {
        Self::from_slice(s)
    }
}

impl<'a, T> From<SSlice<'a, T>> for &'a [T] {
    fn from(s: SSlice<'a, T>) -> Self {
        s.into_slice()
    }
}

impl<'a, T, const N: usize> From<&'a [T; N]> for SSlice<'a, T> {
    fn from(s: &'a [T; N]) -> Self {
        Self::from_slice(s.as_slice())
    }
}

impl<'a, T, const N: usize> From<&'a mut [T; N]> for SSlice<'a, T> {
    fn from(s: &'a mut [T; N]) -> Self {
        Self::from_slice(s.as_slice())
    }
}

impl<'a, T, I> Index<I> for SSlice<'a, T>
where
    I: std::slice::SliceIndex<[T]>,
{
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &I::Output {
        self.as_slice().index(index)
    }
}

impl<'a, T> IntoIterator for SSlice<'a, T> {
    type Item = &'a T;

    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_slice().into_iter()
    }
}

impl<'a, T> Clone for SSlice<'a, T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            length: self.length,
            _phantom_d: PhantomData,
        }
    }
}

impl<'a, T> Copy for SSlice<'a, T> {}

unsafe impl<'a, T> Send for SSlice<'a, T> {}
unsafe impl<'a, T> Sync for SSlice<'a, T> {}
