#[repr(C)]
pub struct Tuple2<T1: Sized, T2>(pub T1, pub T2);

#[repr(C)]
pub struct Tuple3<T1: Sized, T2: Sized, T3>(pub T1, pub T2, pub T3);

#[repr(C)]
pub struct Tuple4<T1: Sized, T2: Sized, T3: Sized, T4>(pub T1, pub T2, pub T3, pub T4);

#[repr(C)]
pub struct Tuple5<T1: Sized, T2: Sized, T3: Sized, T4: Sized, T5>(
    pub T1,
    pub T2,
    pub T3,
    pub T4,
    pub T5,
);

#[repr(C)]
pub struct Tuple6<T1: Sized, T2: Sized, T3: Sized, T4: Sized, T5: Sized, T6>(
    pub T1,
    pub T2,
    pub T3,
    pub T4,
    pub T5,
    pub T6,
);

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

impl<T1: Clone + Sized, T2: Clone + Sized, T3: Clone + Sized, T4: Clone> Clone
    for Tuple4<T1, T2, T3, T4>
{
    fn clone(&self) -> Self {
        Self(
            self.0.clone(),
            self.1.clone(),
            self.2.clone(),
            self.3.clone(),
        )
    }
}

impl<T1: Clone + Sized, T2: Clone + Sized, T3: Clone + Sized, T4: Clone + Sized, T5: Clone> Clone
    for Tuple5<T1, T2, T3, T4, T5>
{
    fn clone(&self) -> Self {
        Self(
            self.0.clone(),
            self.1.clone(),
            self.2.clone(),
            self.3.clone(),
            self.4.clone(),
        )
    }
}

impl<
        T1: Clone + Sized,
        T2: Clone + Sized,
        T3: Clone + Sized,
        T4: Clone + Sized,
        T5: Clone + Sized,
        T6: Clone,
    > Clone for Tuple6<T1, T2, T3, T4, T5, T6>
{
    fn clone(&self) -> Self {
        Self(
            self.0.clone(),
            self.1.clone(),
            self.2.clone(),
            self.3.clone(),
            self.4.clone(),
            self.5.clone(),
        )
    }
}
