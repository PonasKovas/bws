mod impls;

use std::io::{Result, Write};

pub trait ToBytes {
    fn write_to<W: Write>(&self, write: &mut W) -> Result<usize>;
}
