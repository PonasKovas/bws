/// `#[repr(C)]` equivalent of `()`
#[repr(C)]
pub struct BwsUnit([u8; 0]);

pub fn unit() -> BwsUnit {
    BwsUnit([0; 0])
}
