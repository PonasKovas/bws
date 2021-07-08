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
    type Output;

    // panics if key not found
    fn search<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Output
    where
        Self::Key: Borrow<Q>;
}

impl<'a, K: PartialEq, T> LinearSearch<'a> for Vec<(K, T)> {
    type Key = K;
    type Output = T;

    fn search<Q: PartialEq + ?Sized>(&self, key: &Q) -> &Self::Output
    where
        K: Borrow<Q>,
    {
        &self.iter().find(|e| e.0.borrow() == key).unwrap().1
    }
}
