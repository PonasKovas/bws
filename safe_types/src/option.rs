/// FFI-safe `Option` type
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
#[repr(C)]
pub enum SOption<T> {
    Some(T),
    None,
}

impl<T> SOption<T> {
    pub fn from_option(option: Option<T>) -> Self {
        match option {
            Some(v) => Self::Some(v),
            None => Self::None,
        }
    }
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Some(v) => Some(v),
            Self::None => None,
        }
    }
}

impl<T> Default for SOption<T> {
    fn default() -> Self {
        Self::None
    }
}

impl<T> From<Option<T>> for SOption<T> {
    fn from(r: Option<T>) -> Self {
        Self::from_option(r)
    }
}

impl<T> From<SOption<T>> for Option<T> {
    fn from(r: SOption<T>) -> Self {
        r.into_option()
    }
}

impl<'a, T> From<&'a SOption<T>> for SOption<&'a T> {
    fn from(r: &'a SOption<T>) -> Self {
        match r {
            SOption::Some(r) => Self::Some(r),
            SOption::None => Self::None,
        }
    }
}

impl<'a, T> From<&'a mut SOption<T>> for SOption<&'a mut T> {
    fn from(r: &'a mut SOption<T>) -> Self {
        match r {
            SOption::Some(r) => Self::Some(r),
            SOption::None => Self::None,
        }
    }
}

impl<T> From<T> for SOption<T> {
    fn from(r: T) -> Self {
        Self::Some(r)
    }
}
