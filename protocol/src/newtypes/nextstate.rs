use crate::{FromBytes, ToBytes};

#[derive(ToBytes, FromBytes, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextState {
    Status = 1,
    Login = 2,
}
