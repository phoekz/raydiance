use super::*;

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: na::Point3<f32>,
    pub dir: na::UnitVector3<f32>,
}
