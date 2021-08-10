#[repr(C)]
pub enum BwsOption<T> {
    Some(T),
    None,
}

impl<T> BwsOption<T> {
    pub fn from_option(option: Option<T>) -> Self {
        match option {
            Some(r) => Self::Some(r),
            None => Self::None,
        }
    }
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Some(r) => Some(r),
            Self::None => None,
        }
    }
}
