use super::*;

#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub positions: [na::Point3<f32>; 3],
    pub tex_coords: [na::Point2<f32>; 3],
    pub normals: [na::UnitVector3<f32>; 3],
    pub material: u32,
}
