use super::*;

#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub positions: [Point3; 3],
    pub tex_coords: [Point2; 3],
    pub normals: [Normal; 3],
    pub material: u32,
}

impl Triangle {
    pub fn interpolated_tex_coord(&self, barycentrics: &Vec3) -> Point2 {
        Point2::from(
            self.tex_coords[0].coords * barycentrics.x
                + self.tex_coords[1].coords * barycentrics.y
                + self.tex_coords[2].coords * barycentrics.z,
        )
    }

    pub fn interpolated_normal(&self, barycentrics: &Vec3) -> Normal {
        Normal::new_normalize(
            self.normals[0].into_inner() * barycentrics.x
                + self.normals[1].into_inner() * barycentrics.y
                + self.normals[2].into_inner() * barycentrics.z,
        )
    }
}
