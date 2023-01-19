use super::*;

//
// Models
//

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DiffuseModel {
    Lambert,
    Disney,
}

impl Default for DiffuseModel {
    fn default() -> Self {
        DiffuseModel::Disney
    }
}

impl DiffuseModel {
    pub fn name(&self) -> &'static str {
        match self {
            DiffuseModel::Lambert => "Lambert",
            DiffuseModel::Disney => "Disney",
        }
    }
}

//
// Lambertian
//

pub fn lambert(base_color: LinSrgb) -> LinSrgb {
    // Reflectance.
    base_color * INV_PI
}

//
// Disney
//

pub fn disney(
    base_color: LinSrgb,
    roughness: f32,
    l_local: &na::Vector3<f32>,
    v_local: &na::Vector3<f32>,
) -> LinSrgb {
    // Half vector `h`. Also referred to as the microsurface normal.
    let h_local = (v_local + l_local).normalize();

    // Angle of incidences.
    let l_dot_n = cos_theta(&l_local).abs();
    let v_dot_n = cos_theta(&v_local).abs();
    let l_dot_h = l_local.dot(&h_local);

    // Diffuse Fresnel.
    let fl = schlick_fresnel(l_dot_n);
    let fv = schlick_fresnel(v_dot_n);
    let fd90 = 0.5 + 2.0 * l_dot_h.powi(2) * roughness;
    let fd = mix(1.0, fd90, fl) * mix(1.0, fd90, fv);

    // Reflectance.
    base_color * INV_PI * fd
}

//
// Utilities
//

fn mix(x: f32, y: f32, a: f32) -> f32 {
    x * (1.0 - a) + y * a
}

fn cos_theta(vec: &na::Vector3<f32>) -> f32 {
    // In local space, the normal vector is always (0,1,0). This means that the
    // angle of incidence of a vector with respect to the normal is just the
    // y-component of the vector.
    vec.y
}

fn schlick_fresnel(u: f32) -> f32 {
    // Schlick Fresnel approximation.
    let f = (1.0 - u).clamp(0.0, 1.0).powi(5);
    assert!(f >= 0.0, "f must be greater than 0, got {f} instead");
    f
}
