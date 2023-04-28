use super::*;

//
// Notation
//

// wi = incoming direction
// wo = outgoing direction
// wm = microsurface normal, half vector
// wg = (0,1,0) = geometric normal
// r = reflectance
// pdf = probability density function
// theta = angle from geometric normal
// phi = angle around geometric normal on xz plane.

//
// Constants
//

const EPSILON: f32 = 0.001;

//
// Macros
//

macro_rules! assert_range {
    ($val:expr, $min:literal, $max:literal) => {
        assert!(
            ($min..=$max).contains(&$val),
            "value={val_name} must be in between {min}..={max}, got {val} instead`",
            val_name = stringify!($val),
            min = $min,
            max = $max,
            val = $val
        );
    };
}

//
// Aliases
//

type Incoming = LocalVector;
type Outgoing = LocalVector;
type MicrosurfaceNormal = LocalVector;
type UniformSample2D = (f32, f32);
type Reflectance = ColorRgb;
type Pdf = f32;

//
// LocalVector
//

// The local space is a right-handed coordinate system, where geometric_normal =
// (0,1,0). This simplifies a lot of trinogometry, but assumes that all incoming
// and outgoing vectors are in this space.
//
// As an example, cos(θ) = dot(w, wg) in this space is just the y-component of
// w.

#[derive(Clone, Copy, Debug)]
pub struct LocalVector(pub Vec3);

impl LocalVector {
    #[inline]
    pub fn local_from_world(local_from_world: &Mat3, world: &Vec3) -> Self {
        Self((*local_from_world * world).normalize())
    }

    #[inline]
    pub fn world_from_local(&self, world_from_local: &Mat3) -> Normal {
        normal!(*world_from_local * self.0)
    }

    #[inline]
    fn cos_theta(&self) -> f32 {
        self.0.y
    }

    #[inline]
    fn cos2_theta(&self) -> f32 {
        self.0.y * self.0.y
    }

    #[inline]
    fn sin_theta(&self) -> f32 {
        self.sin2_theta().sqrt()
    }

    #[inline]
    fn sin2_theta(&self) -> f32 {
        f32::max(0.0, 1.0 - self.cos2_theta())
    }

    #[inline]
    #[allow(dead_code)]
    fn tan_theta(&self) -> f32 {
        self.sin_theta() / self.cos_theta()
    }

    #[inline]
    fn tan2_theta(&self) -> f32 {
        self.sin2_theta() / self.cos2_theta()
    }

    #[inline]
    fn cos_phi(&self) -> f32 {
        let sin_theta = self.sin_theta();
        if sin_theta == 0.0 {
            1.0
        } else {
            (self.0.x / sin_theta).clamp(-1.0, 1.0)
        }
    }

    #[inline]
    #[allow(dead_code)]
    fn cos2_phi(&self) -> f32 {
        self.cos_phi().powi(2)
    }

    #[inline]
    fn sin_phi(&self) -> f32 {
        let sin_theta = self.sin_theta();
        if sin_theta == 0.0 {
            0.0
        } else {
            (self.0.z / sin_theta).clamp(-1.0, 1.0)
        }
    }

    #[inline]
    #[allow(dead_code)]
    fn sin2_phi(&self) -> f32 {
        self.sin_phi().powi(2)
    }

    #[inline]
    fn microsurface_normal(&self, other: &Self) -> MicrosurfaceNormal {
        Self((self.0 + other.0).normalize())
    }

    #[inline]
    fn same_hemisphere(&self, other: &Self) -> bool {
        self.0.y * other.0.y > 0.0
    }
}

impl std::fmt::Display for LocalVector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{}", self.0.x, self.0.y, self.0.z)
    }
}

//
// Sample
//

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub wi: LocalVector,
    pub r: Reflectance,
    pub pdf: Pdf,
}

impl std::fmt::Display for Sample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "wi={}, r={}, pdf={}", self.wi, self.r, self.pdf)
    }
}

//
// BxDF - Models
//

#[derive(Clone, Copy)]
pub enum Model {
    Lambertian,
    DisneyDiffuse,
    DisneySpecular,
    DisneySheen,
}

impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Lambertian => "lambertian",
                Self::DisneyDiffuse => "disney-diffuse",
                Self::DisneySpecular => "disney-specular",
                Self::DisneySheen => "disney-sheen",
            }
        )
    }
}

//
// BxDF - Trait
//

pub trait Bxdf {
    fn model(&self) -> Model;
    fn eval(&self, wo: &Outgoing, wi: &Incoming) -> Reflectance;
    fn pdf(&self, wo: &Outgoing, wi: &Incoming) -> Pdf;
    fn sample(&self, wo: &Outgoing, u: UniformSample2D) -> Option<Sample>;
}

//
// BxDF - Lambertian
//

#[derive(Clone, Copy, Debug)]
pub struct LambertianParams {
    pub hemisphere: HemisphereSampler,
    pub base_color: ColorRgb,
}

#[derive(Clone, Copy, Debug)]
pub struct Lambertian {
    hemisphere: HemisphereSampler,
    base_color: ColorRgb,
}

impl Lambertian {
    pub fn new(p: &LambertianParams) -> Self {
        Self {
            hemisphere: p.hemisphere,
            base_color: p.base_color,
        }
    }
}

impl Bxdf for Lambertian {
    fn model(&self) -> Model {
        Model::Lambertian
    }

    fn eval(&self, _: &Outgoing, _: &Incoming) -> Reflectance {
        self.base_color * INV_PI
    }

    fn pdf(&self, _: &Outgoing, wi: &Incoming) -> Pdf {
        self.hemisphere.pdf(wi.cos_theta())
    }

    fn sample(&self, wo: &Outgoing, u: UniformSample2D) -> Option<Sample> {
        let wi = LocalVector(*self.hemisphere.sample(u.0, u.1));
        let pdf = self.pdf(wo, &wi);
        if pdf > EPSILON {
            Some(Sample {
                r: self.eval(wo, &wi),
                wi,
                pdf,
            })
        } else {
            None
        }
    }
}

//
// BxDF - Disney Diffuse
//

#[derive(Clone, Copy, Debug)]
pub struct DisneyDiffuseParams {
    pub hemisphere: HemisphereSampler,
    pub base_color: ColorRgb,
    pub roughness: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DisneyDiffuse {
    hemisphere: HemisphereSampler,
    base_color: ColorRgb,
    roughness: f32,
}

impl DisneyDiffuse {
    pub fn new(p: &DisneyDiffuseParams) -> Self {
        assert_range!(p.roughness, 0.0, 1.0);

        Self {
            hemisphere: p.hemisphere,
            base_color: p.base_color,
            roughness: p.roughness,
        }
    }
}

impl Bxdf for DisneyDiffuse {
    fn model(&self) -> Model {
        Model::DisneyDiffuse
    }

    fn eval(&self, wo: &Outgoing, wi: &Incoming) -> Reflectance {
        // Angles.
        let cos_theta_i = wi.cos_theta().abs().max(EPSILON);
        let cos_theta_o = wo.cos_theta().abs().max(EPSILON);

        // Diffuse Fresnel.
        let wm = &wo.microsurface_normal(wi);
        let dot_im = wi.0.dot(&wm.0);
        let fd_90 = 0.5 + 2.0 * self.roughness * dot_im.powi(2);
        let fd_i = 1.0 + ((fd_90 - 1.0) * (1.0 - cos_theta_i).powi(5));
        let fd_o = 1.0 + ((fd_90 - 1.0) * (1.0 - cos_theta_o).powi(5));
        let fd = fd_i * fd_o;

        // Diffuse reflectance.
        self.base_color * INV_PI * fd
    }

    fn pdf(&self, _: &Outgoing, wi: &Incoming) -> Pdf {
        self.hemisphere.pdf(wi.cos_theta().abs())
    }

    fn sample(&self, wo: &Outgoing, u: UniformSample2D) -> Option<Sample> {
        let wi = LocalVector(*self.hemisphere.sample(u.0, u.1));
        let pdf = self.pdf(wo, &wi);
        if pdf > EPSILON {
            Some(Sample {
                r: self.eval(wo, &wi),
                wi,
                pdf,
            })
        } else {
            None
        }
    }
}

//
// BxDF - Disney Specular
//

//
// Sources:
//
// Understanding the Masking-Shadowing Function in Microfacet-Based BRDFs
// Eric Heitz and Eugene D’Eon, 2014
// https://jcgt.org/published/0003/02/03/paper.pdf
//
// A Simpler and Exact Sampling Routine for the GGX Distribution of Visible Normals
// Eric Heitz, 2017
// https://hal.science/hal-01509746/document
//

#[derive(Clone, Copy, Debug)]
pub struct DisneySpecularParams {
    pub base_color: ColorRgb,
    pub metallic: f32,
    pub specular: f32,
    pub specular_tint: f32,
    pub roughness: f32,
    pub anisotropic: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DisneySpecular {
    specular_color: ColorRgb,
    alpha_x: f32,
    alpha_y: f32,
}

impl DisneySpecular {
    pub fn new(p: &DisneySpecularParams) -> Self {
        assert_range!(p.metallic, 0.0, 1.0);
        assert_range!(p.specular, 0.0, 1.0);
        assert_range!(p.specular_tint, 0.0, 1.0);
        assert_range!(p.roughness, 0.0, 1.0);
        assert_range!(p.anisotropic, 0.0, 1.0);

        let specular_color = {
            let luminance = p.base_color.luminance();
            let tint_color = if luminance > 0.0 {
                p.base_color / luminance
            } else {
                ColorRgb::WHITE
            };
            let metallic_color =
                p.specular * 0.08 * lerp_color(&ColorRgb::WHITE, &tint_color, p.specular_tint);
            lerp_color(&metallic_color, &p.base_color, p.metallic)
        };

        let aspect = (1.0 - p.anisotropic * 0.9).sqrt();
        let alpha_x = f32::max(0.001, p.roughness.powi(2) / aspect);
        let alpha_y = f32::max(0.001, p.roughness.powi(2) * aspect);

        Self {
            specular_color,
            alpha_x,
            alpha_y,
        }
    }

    fn ggx_d(&self, wm: &MicrosurfaceNormal) -> f32 {
        let tan2_theta = wm.tan2_theta();
        if !tan2_theta.is_finite() {
            return 0.0;
        }
        let cos4_theta = wm.cos2_theta().powi(2);
        let cos_phi = wm.cos_phi();
        let sin_phi = wm.sin_phi();
        let alpha_x = self.alpha_x;
        let alpha_y = self.alpha_y;
        let e = tan2_theta * ((cos_phi / alpha_x).powi(2) + (sin_phi / alpha_y).powi(2));
        1.0 / (PI * alpha_x * alpha_y * cos4_theta * (1.0 + e).powi(2))
    }

    fn ggx_lambda(&self, w: &LocalVector) -> f32 {
        let tan2_theta = w.tan2_theta();
        if !tan2_theta.is_finite() {
            return 0.0;
        }
        let cos_phi = w.cos_phi();
        let sin_phi = w.sin_phi();
        let alpha_x = self.alpha_x;
        let alpha_y = self.alpha_y;
        let alpha2 = (cos_phi * alpha_x).powi(2) + (sin_phi * alpha_y).powi(2);
        ((1.0 + alpha2 + tan2_theta).sqrt() - 1.0) / 2.0
    }

    fn ggx_g1(&self, w: &LocalVector) -> f32 {
        1.0 / (1.0 + self.ggx_lambda(w))
    }

    // Height-correlated masking-masking function.
    fn ggx_g(&self, wo: &Outgoing, wi: &Incoming) -> f32 {
        1.0 / (1.0 + self.ggx_lambda(wo) + self.ggx_lambda(wi))
    }

    fn ggx_sample_wm(&self, wo: &Outgoing, u: UniformSample2D) -> MicrosurfaceNormal {
        // Stretch.
        let v = vector![self.alpha_x * wo.0.x, wo.0.y, self.alpha_y * wo.0.z].normalize();
        let v = if v.y < 0.0 { -v } else { v };

        // Orthonormal basis.
        let t1 = if v.y < 0.9999 {
            v.cross(&Y_AXIS).normalize()
        } else {
            X_AXIS
        };
        let t2 = t1.cross(&v);

        // Sample point with polar coordinates (r, phi).
        let a = 1.0 / (1.0 + v.y);
        let r = u.0.sqrt();
        let phi = if u.1 < a {
            u.1 / a * PI
        } else {
            PI + (u.1 - a) / (1.0 - a) * PI
        };
        let p1 = r * phi.cos();
        let p2 = r * phi.sin() * if u.1 < a { 1.0 } else { v.y };
        let p3 = (1.0 - p1 * p1 - p2 * p2).max(0.0).sqrt();

        // Compute normal.
        let n = p1 * t1 + p2 * t2 + p3 * v;

        // Unstretch.
        LocalVector(vector![self.alpha_x * n.x, n.y, self.alpha_y * n.z].normalize())
    }

    fn fresnel(&self, wo: &Outgoing, wm: &MicrosurfaceNormal) -> ColorRgb {
        let dot_om = wo.0.dot(&wm.0);
        let fresnel = (1.0 - dot_om).clamp(0.0, 1.0).powi(5);
        lerp_color(&self.specular_color, &ColorRgb::WHITE, fresnel)
    }
}

impl Bxdf for DisneySpecular {
    fn model(&self) -> Model {
        Model::DisneySpecular
    }

    fn eval(&self, wo: &Outgoing, wi: &Incoming) -> Reflectance {
        // Angles.
        let cos_theta_i = wi.cos_theta().abs().max(EPSILON);
        let cos_theta_o = wo.cos_theta().abs().max(EPSILON);

        // Cook-Torrance microfacet model.
        let wm = &wo.microsurface_normal(wi);
        let d = self.ggx_d(wm);
        let g = self.ggx_g(wo, wi);
        let f = self.fresnel(wo, wm);
        d * g * f / (4.0 * cos_theta_i * cos_theta_o)
    }

    fn pdf(&self, wo: &Outgoing, wi: &Incoming) -> Pdf {
        let wm = &wo.microsurface_normal(wi);
        let g1 = self.ggx_g1(wo);
        let d = self.ggx_d(wm);
        let dot_om = wo.0.dot(&wm.0).abs().max(EPSILON);
        let cos_theta_o = wo.cos_theta().abs().max(EPSILON);
        let d_pdf = g1 / cos_theta_o * d * dot_om;
        d_pdf / (4.0 * dot_om)
    }

    fn sample(&self, wo: &Outgoing, u: UniformSample2D) -> Option<Sample> {
        let wm = self.ggx_sample_wm(wo, u);
        let wi = LocalVector(reflect_vector(&wo.0, &wm.0));
        if !wo.same_hemisphere(&wi) {
            return None;
        }
        let pdf = self.pdf(wo, &wi);
        if pdf > EPSILON {
            Some(Sample {
                r: self.eval(wo, &wi),
                wi,
                pdf,
            })
        } else {
            None
        }
    }
}

//
// BxDF - Disney Sheen
//

#[derive(Clone, Copy, Debug)]
pub struct DisneySheenParams {
    pub hemisphere: HemisphereSampler,
    pub base_color: ColorRgb,
    pub sheen: f32,
    pub sheen_tint: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DisneySheen {
    hemisphere: HemisphereSampler,
    sheen_color: ColorRgb,
}

impl DisneySheen {
    pub fn new(p: &DisneySheenParams) -> Self {
        // Todo: Shared with DisneySpecular.
        let luminance = p.base_color.luminance();
        let tint_color = if luminance > 0.0 {
            p.base_color / luminance
        } else {
            ColorRgb::WHITE
        };

        Self {
            sheen_color: p.sheen * lerp_color(&ColorRgb::WHITE, &tint_color, p.sheen_tint),
            hemisphere: p.hemisphere,
        }
    }
}

impl Bxdf for DisneySheen {
    fn model(&self) -> Model {
        Model::DisneySheen
    }

    fn eval(&self, wo: &Outgoing, wi: &Incoming) -> Reflectance {
        let wm = wo.microsurface_normal(wi);
        let dot_im = wi.0.dot(&wm.0);
        let fresnel = (1.0 - dot_im).clamp(0.0, 1.0).powi(5);
        self.sheen_color * fresnel
    }

    fn pdf(&self, _: &Outgoing, wi: &Incoming) -> Pdf {
        self.hemisphere.pdf(wi.cos_theta().abs())
    }

    fn sample(&self, wo: &Outgoing, u: UniformSample2D) -> Option<Sample> {
        let wi = LocalVector(*self.hemisphere.sample(u.0, u.1));
        let pdf = self.pdf(wo, &wi);
        if pdf > EPSILON {
            Some(Sample {
                r: self.eval(wo, &wi),
                wi,
                pdf,
            })
        } else {
            None
        }
    }
}
