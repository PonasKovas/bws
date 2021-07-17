#[derive(Copy, Clone, PartialEq, Debug)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// width
    pub w: f64,
    /// height
    pub h: f64,
    /// length
    pub l: f64,
}

pub fn is_colliding(box1: BoundingBox, box2: BoundingBox) -> bool {
    box1.x < (box2.x + box2.w)
        && (box1.x + box1.w) > box2.x
        && box1.y < (box2.y + box2.h)
        && (box1.y + box1.h) > box2.y
        && box1.z < (box2.z + box2.l)
        && (box1.z + box1.l) > box2.z
}
