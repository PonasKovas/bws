use super::SUnit;

/// FFI-safe equivalent of `Result<T, E>`
#[repr(C)]
pub enum SResult<T = SUnit, E = SUnit> {
    Ok(T),
    Err(E),
}

impl<T, E> SResult<T, E> {
    pub fn from_result(r: Result<T, E>) -> Self {
        match r {
            Ok(v) => Self::Ok(v),
            Err(v) => Self::Err(v),
        }
    }
    pub fn into_result(self) -> Result<T, E> {
        match self {
            Self::Ok(v) => Ok(v),
            Self::Err(v) => Err(v),
        }
    }
}

impl<T, E> From<Result<T, E>> for SResult<T, E> {
    fn from(r: Result<T, E>) -> Self {
        Self::from_result(r)
    }
}

impl<T, E> From<SResult<T, E>> for Result<T, E> {
    fn from(r: SResult<T, E>) -> Self {
        r.into_result()
    }
}
