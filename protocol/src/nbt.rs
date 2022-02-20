super::cfg_ser! {
    mod read;
    mod write;
}

use std::ops::{Deref, DerefMut, Index, IndexMut};

#[cfg(feature = "ffi_safe")]
type Vec<T> = safe_types::std::vec::SVec<T>;

#[cfg(feature = "ffi_safe")]
type String = safe_types::std::string::SString;

#[cfg(feature = "ffi_safe")]
type Tuple2<T1, T2> = safe_types::STuple2<T1, T2>;
#[cfg(not(feature = "ffi_safe"))]
type Tuple2<T1, T2> = (T1, T2);

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ffi_safe", repr(C))]
pub enum NbtTag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(NbtList),
    Compound(NbtCompound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ffi_safe", repr(C))]
pub struct NbtList(pub Vec<NbtTag>);

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "ffi_safe", repr(C))]
pub struct NbtCompound(pub Vec<Tuple2<String, NbtTag>>);

impl NbtList {
    /// Creates a new empty `NbtList`
    pub fn new() -> Self {
        Self(Vec::new())
    }
    /// Returns the number of elements in this `NbtList`
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Adds an element to this `NbtList`
    pub fn push(&mut self, value: NbtTag) {
        self.0.push(value)
    }
}

impl Deref for NbtList {
    type Target = Vec<NbtTag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for NbtList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Index<usize> for NbtList {
    type Output = NbtTag;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
impl IndexMut<usize> for NbtList {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl NbtCompound {
    /// Creates a new empty `NbtCompound`
    pub fn new() -> Self {
        Self(Vec::new())
    }
    /// Checks if a key is present in the `NbtCompound`
    pub fn contains_key(&self, key: &str) -> bool {
        #[cfg(feature = "ffi_safe")]
        let v = self.0.as_vec();
        #[cfg(not(feature = "ffi_safe"))]
        let v = &self.0;

        v.iter().find(|t| &t.0 == key).is_some()
    }
    /// Returns the value of the given key, if there is one
    pub fn get<'a, 'b>(&'a self, key: &'b str) -> Option<&'a NbtTag> {
        #[cfg(feature = "ffi_safe")]
        let v = self.0.as_vec();
        #[cfg(not(feature = "ffi_safe"))]
        let v = &self.0;

        v.iter().position(|t| &t.0 == key).map(|id| &self.0[id].1)
    }
    /// Returns a mutable reference to the value of the given key, if there is one
    pub fn get_mut<'a, 'b>(&'a mut self, key: &'b str) -> Option<&'a mut NbtTag> {
        #[cfg(feature = "ffi_safe")]
        let v = self.0.as_vec();
        #[cfg(not(feature = "ffi_safe"))]
        let v = &self.0;

        v.iter()
            .position(|t| &t.0 == key)
            .map(move |id| &mut self.0[id].1)
    }
    /// Inserts a new element
    pub fn insert(&mut self, key: String, val: NbtTag) {
        #[cfg(feature = "ffi_safe")]
        self.0.push(safe_types::STuple2(key, val));
        #[cfg(not(feature = "ffi_safe"))]
        self.0.push((key, val));
    }
    /// Returns true if the `NbtCompound` has no elements
    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }
    /// Returns the number of elements
    pub fn len(&self) -> usize {
        self.0.len()
    }
}
