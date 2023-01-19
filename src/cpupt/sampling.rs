use super::*;

pub struct UniformSampler {
    state: rand_pcg::Pcg64Mcg,
    distribution: rand::distributions::Uniform<f32>,
}

impl UniformSampler {
    pub fn new() -> Self {
        Self::new_with_seed(0)
    }

    pub fn new_with_seed(seed: u64) -> Self {
        Self {
            state: rand_pcg::Pcg64Mcg::seed_from_u64(seed),
            distribution: rand::distributions::Uniform::new_inclusive(0.0, 1.0),
        }
    }

    pub fn sample(&mut self) -> f32 {
        self.distribution.sample(&mut self.state)
    }
}

pub fn primary_ray(
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

pub struct OrthonormalBasis {
    world_from_local: na::Matrix3<f32>,
    local_from_world: na::Matrix3<f32>,
}

impl OrthonormalBasis {
    pub fn new(n: &na::UnitVector3<f32>) -> Self {
        // Implementation based on "Building an Orthonormal Basis, Revisited".
        // https://graphics.pixar.com/library/OrthonormalB/paper.pdf
        let sign = f32::copysign(1.0, n.z);
        let a = -1.0 / (sign + n.z);
        let b = n.x * n.y * a;
        let t = na::Unit::new_normalize(na::vector![
            1.0 + sign * n.x * n.x * a,
            sign * b,
            -sign * n.x
        ]);
        let b = na::Unit::new_normalize(na::vector![b, sign + n.y * n.y * a, -n.y]);

        let world_from_local =
            na::Matrix3::from_columns(&[t.into_inner(), n.into_inner(), b.into_inner()]);
        let local_from_world = world_from_local.transpose();
        Self {
            world_from_local,
            local_from_world,
        }
    }

    pub fn local_from_world(&self) -> &na::Matrix3<f32> {
        &self.local_from_world
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HemisphereSampler {
    Uniform,
    Cosine,
}

impl HemisphereSampler {
    pub fn sample(self, onb: &OrthonormalBasis, s: f32, t: f32) -> na::UnitVector3<f32> {
        let dir = match self {
            HemisphereSampler::Uniform => hemisphere_uniform(s, t),
            HemisphereSampler::Cosine => hemisphere_cosine(s, t),
        };
        na::Unit::new_normalize(onb.world_from_local * dir)
    }

    pub fn pdf(self, cos_theta: f32) -> f32 {
        let pdf = match self {
            HemisphereSampler::Uniform => hemisphere_uniform_pdf(),
            HemisphereSampler::Cosine => hemisphere_cosine_pdf(cos_theta),
        };
        assert!(
            pdf >= 0.0 && pdf <= 1.0,
            "pdf must be between 0..1, got {pdf} instead"
        );
        pdf
    }

    pub fn name(self) -> &'static str {
        match self {
            HemisphereSampler::Uniform => "Uniform",
            HemisphereSampler::Cosine => "Cosine",
        }
    }
}

impl Default for HemisphereSampler {
    fn default() -> Self {
        Self::Cosine
    }
}

impl std::fmt::Display for HemisphereSampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

fn hemisphere_uniform(s: f32, t: f32) -> na::Vector3<f32> {
    let u = TAU * s;
    let v = f32::sqrt(f32::max(0.0, 1.0 - t * t));
    let px = v * f32::cos(u);
    let py = t;
    let pz = v * f32::sin(u);
    na::vector![px, py, pz]
}

fn hemisphere_uniform_pdf() -> f32 {
    1.0 / (2.0 * PI)
}

fn concentric_disk(s: f32, t: f32) -> na::Vector2<f32> {
    let s = 2.0 * s - 1.0;
    let t = 2.0 * t - 1.0;
    if s == 0.0 && t == 0.0 {
        return na::vector![0.0, 0.0];
    }

    let (r, theta) = if f32::abs(s) > f32::abs(t) {
        (s, (PI / 4.0) * (t / s))
    } else {
        (t, (PI / 2.0) - (PI / 4.0) * (s / t))
    };

    na::vector![r * f32::cos(theta), r * f32::sin(theta)]
}

fn hemisphere_cosine(s: f32, t: f32) -> na::Vector3<f32> {
    let d = concentric_disk(s, t);
    let y = f32::sqrt(f32::max(0.0, 1.0 - d.x * d.x - d.y * d.y));
    na::vector![d.x, y, d.y]
}

fn hemisphere_cosine_pdf(cos_theta: f32) -> f32 {
    cos_theta / PI
}
