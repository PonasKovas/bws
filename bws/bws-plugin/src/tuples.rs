#[repr(C)]
pub struct Tuple2<T1: Sized, T2>(pub T1, pub T2);

#[repr(C)]
pub struct Tuple3<T1: Sized, T2: Sized, T3>(pub T1, pub T2, pub T3);

impl<T1: Clone + Sized, T2: Clone> Clone for Tuple2<T1, T2> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}

impl<T1: Clone + Sized, T2: Clone + Sized, T3: Clone> Clone for Tuple3<T1, T2, T3> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone(), self.2.clone())
    }
}
