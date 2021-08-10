#[repr(C)]
pub struct Unit([u8; 0]);

pub fn unit() -> Unit {
    Unit([0; 0])
}
