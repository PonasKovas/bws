use std::borrow::Borrow;

pub trait InRange {
    fn in_range(self, begin: Self, end: Self) -> bool;
}

impl InRange for f32 {
    fn in_range(self, begin: f32, end: f32) -> bool {
        self >= begin && self < end
    }
}

pub trait LinearSearch<'a> {
    type Key;
    type Value;

    /// panics if key not found
    fn search<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Value
    where
        Self::Key: Borrow<Q>;

    /// panics if key not found
    fn search_by_val<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Key
    where
        Self::Value: Borrow<Q>;
}

impl<'a, K, T> LinearSearch<'a> for Vec<(K, T)> {
    type Key = K;
    type Value = T;

    fn search<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Value
    where
        K: Borrow<Q>,
    {
        &self.iter().find(|e| e.0.borrow() == key).unwrap().1
    }

    fn search_by_val<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Key
    where
        T: Borrow<Q>,
    {
        &self.iter().find(|e| e.1.borrow() == key).unwrap().0
    }
}

impl<'a, K: rkyv::Archive, T: rkyv::Archive> LinearSearch<'a>
    for rkyv::vec::ArchivedVec<crate::data::ArchivedTuple<K, T>>
{
    type Key = K::Archived;
    type Value = T::Archived;

    fn search<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Value
    where
        K::Archived: Borrow<Q>,
    {
        &self.iter().find(|e| e.0.borrow() == key).unwrap().1
    }

    fn search_by_val<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Key
    where
        T::Archived: Borrow<Q>,
    {
        &self.iter().find(|e| e.1.borrow() == key).unwrap().0
    }
}
