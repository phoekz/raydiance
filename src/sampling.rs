use super::*;

pub fn camera_ray_uniform(
    (pixel_x, pixel_y): (u32, u32),
    (image_w, image_h): (u32, u32),
    camera_position: &na::Point3<f32>,
    world_from_clip: &na::Matrix4<f32>,
    s: f32,
    t: f32,
) -> Ray {
    // Center pixel.
    let px = pixel_x as f32 + s;
    let py = pixel_y as f32 + t;

    // Normalize 0..window -> 0..1.
    let px = px / image_w as f32;
    let py = py / image_h as f32;

    // Flip Y to match Vulkan screen space.
    let py = 1.0 - py;

    // Scale 0..1 -> -1..1.
    let px = 2.0 * px - 1.0;
    let py = 2.0 * py - 1.0;

    // Transform.
    let pxyzw = world_from_clip * na::vector![px, py, 1.0, 1.0];
    let pxyz = pxyzw.fixed_rows::<3>(0);
    let p = na::Point3::from(pxyz / pxyzw.w);

    Ray {
        origin: *camera_position,
        dir: na::Unit::new_normalize(p - camera_position),
    }
}

pub fn orthonormal_basis(n: &na::Vector3<f32>) -> (na::Vector3<f32>, na::Vector3<f32>) {
    // Implementation based on "Building an Orthonormal Basis, Revisited".
    // https://graphics.pixar.com/library/OrthonormalB/paper.pdf
    let sign = f32::copysign(1.0, n.z);
    let a = -1.0 / (sign + n.z);
    let b = n.x * n.y * a;
    (
        na::vector![1.0 + sign * n.x * n.x * a, sign * b, -sign * n.x],
        na::vector![b, sign + n.y * n.y * a, -n.y],
    )
}

pub fn hemisphere_uniform(s: f32, t: f32) -> na::Vector3<f32> {
    let u = TAU * s;
    let v = f32::sqrt(f32::max(0.0, 1.0 - t * t));
    let px = v * f32::cos(u);
    let py = t;
    let pz = v * f32::sin(u);
    na::vector![px, py, pz]
}

pub fn direction_uniform(n: &na::Vector3<f32>, s: f32, t: f32) -> na::UnitVector3<f32> {
    let (b1, b2) = orthonormal_basis(n);
    let dir = hemisphere_uniform(s, t);
    na::Unit::new_normalize(na::vector![
        dir.dot(&na::vector![b1.x, n.x, b2.x]),
        dir.dot(&na::vector![b1.y, n.y, b2.y]),
        dir.dot(&na::vector![b1.z, n.z, b2.z])
    ])
}
