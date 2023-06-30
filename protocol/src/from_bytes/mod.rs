use std::io::{Read, Result};

mod impls;

pub trait FromBytes {
    fn read_from<R: Read>(read: &mut R) -> Result<Self>
    where
        Self: Sized;
}
