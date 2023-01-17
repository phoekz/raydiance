use super::*;

type Position = na::Point3<f32>;
type TexCoord = na::Point2<f32>;
type Normal = na::UnitVector3<f32>;

#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub positions: [Position; 3],
    pub tex_coords: [TexCoord; 3],
    pub normals: [Normal; 3],
    pub material: u32,
}

impl Triangle {
    pub fn interpolated_tex_coord(&self, barycentrics: &na::Vector3<f32>) -> TexCoord {
        TexCoord::from(
            self.tex_coords[0].coords * barycentrics.x
                + self.tex_coords[1].coords * barycentrics.y
                + self.tex_coords[2].coords * barycentrics.z,
        )
    }

    pub fn interpolated_normal(&self, barycentrics: &na::Vector3<f32>) -> Normal {
        Normal::new_normalize(
            self.normals[0].into_inner() * barycentrics.x
                + self.normals[1].into_inner() * barycentrics.y
                + self.normals[2].into_inner() * barycentrics.z,
        )
    }
}
