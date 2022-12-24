use std::fmt::Debug;

/// `#[repr(C)]` version of `(A, B)`
#[repr(C)]
pub struct STuple2<A, B>(pub A, pub B);

impl<A: PartialEq, B: PartialEq> PartialEq for STuple2<A, B> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == self.1
    }
}
impl<A: Clone, B: Clone> Clone for STuple2<A, B> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}
impl<A: Debug, B: Debug> Debug for STuple2<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&(&self.0, &self.1), f)
    }
}
