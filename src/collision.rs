pub fn is_colliding(
    x1: f64,
    y1: f64,
    z1: f64,
    w1: f64,
    h1: f64,
    l1: f64,
    x2: f64,
    y2: f64,
    z2: f64,
    w2: f64,
    h2: f64,
    l2: f64,
) -> bool {
    x1 < (x2 + w2)
        && (x1 + w1) > x2
        && y1 < (y2 + h2)
        && (y1 + h1) > y2
        && z1 < (z2 + l2)
        && (z1 + l1) > z2
}
