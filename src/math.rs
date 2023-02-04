use super::*;

//
// Linear algebra
//

pub use na::vector;

pub type Vec2 = na::Vector2<f32>;
pub type Vec3 = na::Vector3<f32>;
pub type Vec4 = na::Vector4<f32>;

pub type Vec3b = na::Vector3<bool>;
pub type Vec3u = na::Vector3<u32>;

pub type Mat3 = na::Matrix3<f32>;
pub type Mat4 = na::Matrix4<f32>;

pub type Point2 = na::Point2<f32>;
pub type Point3 = na::Point3<f32>;

pub type Normal = na::UnitVector3<f32>;

pub type Perspective3 = na::Perspective3<f32>;

pub const X_AXIS: Vec3 = vector![1.0, 0.0, 0.0];
pub const Y_AXIS: Vec3 = vector![0.0, 1.0, 0.0];
pub const Z_AXIS: Vec3 = vector![0.0, 0.0, 1.0];

#[macro_export]
macro_rules! normal {
    ($v:expr) => {
        na::Unit::new_normalize($v)
    };

    ($x:expr, $y:expr, $z:expr) => {
        na::Unit::new_normalize(na::Vector3::<f32>::new($x, $y, $z))
    };
}

//
// Color
//

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, Pod, Zeroable)]
pub struct ColorRgb([f32; 3]);

impl ColorRgb {
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0);
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0);

    #[inline]
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self([r, g, b])
    }

    #[inline]
    pub const fn red(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub const fn green(&self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub const fn blue(&self) -> f32 {
        self.0[2]
    }

    #[inline]
    pub fn is_finite(&self) -> bool {
        self.0[0].is_finite() && self.0[1].is_finite() && self.0[2].is_finite()
    }

    #[inline]
    pub fn to_srgb_bytes(self) -> [u8; 3] {
        use palette::{LinSrgb, Pixel, Srgb};
        let linear = LinSrgb::new(self.red(), self.green(), self.blue());
        let srgb = Srgb::from_linear(linear);
        let bytes: [u8; 3] = srgb.into_format().into_raw();
        bytes
    }
}

impl std::ops::AddAssign for ColorRgb {
    fn add_assign(&mut self, rhs: Self) {
        self.0[0] += rhs.0[0];
        self.0[1] += rhs.0[1];
        self.0[2] += rhs.0[2];
    }
}

impl std::ops::Mul<f32> for ColorRgb {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self([self.0[0] * rhs, self.0[1] * rhs, self.0[2] * rhs])
    }
}

impl std::ops::Mul<ColorRgb> for f32 {
    type Output = ColorRgb;

    fn mul(self, rhs: ColorRgb) -> Self::Output {
        ColorRgb([rhs.0[0] * self, rhs.0[1] * self, rhs.0[2] * self])
    }
}

impl std::ops::MulAssign for ColorRgb {
    fn mul_assign(&mut self, rhs: Self) {
        self.0[0] *= rhs.0[0];
        self.0[1] *= rhs.0[1];
        self.0[2] *= rhs.0[2];
    }
}

impl std::ops::Div<f32> for ColorRgb {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self([self.0[0] / rhs, self.0[1] / rhs, self.0[2] / rhs])
    }
}

impl std::fmt::Display for ColorRgb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{}", self.red(), self.green(), self.blue())
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, Pod, Zeroable)]
pub struct ColorRgba([f32; 4]);

impl ColorRgba {
    #[inline]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self([r, g, b, a])
    }

    #[inline]
    pub const fn red(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub const fn green(&self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub const fn blue(&self) -> f32 {
        self.0[2]
    }

    #[inline]
    #[allow(dead_code)]
    pub const fn alpha(&self) -> f32 {
        self.0[3]
    }

    #[inline]
    pub const fn rgb(&self) -> ColorRgb {
        ColorRgb::new(self.red(), self.green(), self.blue())
    }
}

//
// Interpolation
//

pub fn lerp_scalar<T: num::Float>(a: T, b: T, t: T) -> T {
    a * (T::one() - t) + b * t
}

pub fn lerp_color(a: &ColorRgb, b: &ColorRgb, t: f32) -> ColorRgb {
    ColorRgb::new(
        lerp_scalar(a.red(), b.red(), t),
        lerp_scalar(a.green(), b.green(), t),
        lerp_scalar(a.blue(), b.blue(), t),
    )
}

//
// Geometric
//

pub fn reflect_vector(v: &Vec3, n: &Vec3) -> Vec3 {
    (2.0 * v.dot(n) * n - v).normalize()
}

//
// Bounding volumes
//

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingSphere {
    center: Point3,
    radius: f32,
}

impl BoundingSphere {
    #[inline]
    pub fn center(&self) -> Point3 {
        self.center
    }

    #[inline]
    pub fn radius(&self) -> f32 {
        self.radius
    }
}

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    extents: [Point3; 2],
}

impl Aabb {
    #[inline]
    pub fn new() -> Self {
        Self {
            extents: [
                Vec3::repeat(f32::MAX).into(),
                Vec3::repeat(-f32::MAX).into(),
            ],
        }
    }

    #[inline]
    pub fn from_min_max(min: &Point3, max: &Point3) -> Self {
        Self {
            extents: [*min, *max],
        }
    }

    pub fn from_points<'a, Iter>(points: Iter) -> Self
    where
        Iter: IntoIterator<Item = &'a Point3>,
    {
        let mut aabb = Self::new();
        for point in points {
            aabb.extend(point);
        }
        aabb
    }

    #[inline]
    pub fn min(&self) -> Point3 {
        self.extents[0]
    }

    #[inline]
    pub fn max(&self) -> Point3 {
        self.extents[1]
    }

    #[inline]
    pub fn center(&self) -> Point3 {
        na::center(&self.min(), &self.max())
    }

    #[inline]
    pub fn extents(&self) -> Vec3 {
        self.max() - self.min()
    }

    pub fn extend(&mut self, point: &Point3) {
        self.extents[0] = self.min().coords.inf(&point.coords).into();
        self.extents[1] = self.max().coords.sup(&point.coords).into();
    }

    pub fn merge(&mut self, other: &Aabb) {
        self.extents[0] = self.min().inf(&other.min());
        self.extents[1] = self.max().sup(&other.max());
    }

    pub fn merged(&self, other: &Aabb) -> Self {
        Self {
            extents: [self.min().inf(&other.min()), self.max().sup(&other.max())],
        }
    }

    pub fn bounding_sphere(&self) -> BoundingSphere {
        let center = self.center();
        let radius = na::distance(&center, &self.max());
        BoundingSphere { center, radius }
    }
}

impl Default for Aabb {
    fn default() -> Self {
        Self::new()
    }
}

//
// Tests
//

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_ulps_eq;

    #[test]
    fn test_normal_macro() {
        let result = 0.57735026;
        let normal = normal![1.0, 1.0, 1.0];
        assert_ulps_eq!(normal.x, result, max_ulps = 1);
        assert_ulps_eq!(normal.y, result, max_ulps = 1);
        assert_ulps_eq!(normal.z, result, max_ulps = 1);

        let normal = normal![vector![1.0, 1.0, 1.0]];
        assert_ulps_eq!(normal.x, result, max_ulps = 1);
        assert_ulps_eq!(normal.y, result, max_ulps = 1);
        assert_ulps_eq!(normal.z, result, max_ulps = 1);
    }

    #[test]
    fn test_lerp_scalar() {
        assert_ulps_eq!(lerp_scalar(0.0, 1.0, 0.0), 0.0, max_ulps = 1);
        assert_ulps_eq!(lerp_scalar(0.0, 1.0, 0.5), 0.5, max_ulps = 1);
        assert_ulps_eq!(lerp_scalar(0.0, 1.0, 1.0), 1.0, max_ulps = 1);
    }

    #[test]
    fn test_lerp_color() {
        let a = ColorRgb::BLACK;
        let b = ColorRgb::WHITE;
        let c = lerp_color(&a, &b, 0.5);
        assert_ulps_eq!(c.red(), 0.5, max_ulps = 1);
        assert_ulps_eq!(c.green(), 0.5, max_ulps = 1);
        assert_ulps_eq!(c.blue(), 0.5, max_ulps = 1);
    }

    #[test]
    fn test_reflect_vector() {
        let v = vector![1.0, 1.0, 0.0].normalize();
        let n = vector![0.0, 1.0, 0.0];
        let r = reflect_vector(&v, &n);
        assert_ulps_eq!(v.x, -r.x, max_ulps = 1);
        assert_ulps_eq!(v.y, r.y, max_ulps = 1);
        assert_ulps_eq!(v.z, r.z, max_ulps = 1);
    }
}
