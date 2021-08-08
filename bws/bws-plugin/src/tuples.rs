#[repr(C)]
pub struct Tuple2<T1: Sized, T2>(pub T1, pub T2);

#[repr(C)]
pub struct Tuple3<T1: Sized, T2: Sized, T3>(pub T1, pub T2, pub T3);
