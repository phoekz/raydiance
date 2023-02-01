use super::*;

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: Point3,
    pub dir: Normal,
}
