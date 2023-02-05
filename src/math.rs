use super::*;

//
// Linear algebra
//

pub use na::{matrix, vector};

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

macro_rules! normal {
    ($v:expr) => {
        na::Unit::new_normalize($v)
    };

    ($x:expr, $y:expr, $z:expr) => {
        na::Unit::new_normalize(na::Vector3::<f32>::new($x, $y, $z))
    };
}
pub(crate) use normal;

//
// Color
//

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug, Pod, Zeroable, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ColorRgb([f32; 3]);

impl ColorRgb {
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0);
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0);

    #[inline]
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self([r, g, b])
    }

    #[inline]
    pub const fn from_array(arr: [f32; 3]) -> Self {
        Self(arr)
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
        assert!((0.0..=1.0).contains(&self.red()));
        assert!((0.0..=1.0).contains(&self.green()));
        assert!((0.0..=1.0).contains(&self.blue()));
        let linear = LinSrgb::new(self.red(), self.green(), self.blue());
        let srgb = Srgb::from_linear(linear);
        let bytes: [u8; 3] = srgb.into_format().into_raw();
        bytes
    }

    #[inline]
    pub fn clamp01(self) -> Self {
        Self([
            self.0[0].clamp(0.0, 1.0),
            self.0[1].clamp(0.0, 1.0),
            self.0[2].clamp(0.0, 1.0),
        ])
    }

    pub fn tonemap(self) -> Self {
        // https://knarkowicz.wordpress.com/2016/01/06/aces-filmic-tone-mapping-curve/
        let aces = |x: f32| {
            let a = 2.51;
            let b = 0.03;
            let c = 2.43;
            let d = 0.59;
            let e = 0.14;
            f32::clamp((x * (a * x + b)) / (x * (c * x + d) + e), 0.0, 1.0)
        };
        Self::new(aces(self.red()), aces(self.green()), aces(self.blue()))
    }

    pub fn raw_mut(&mut self) -> &mut [f32; 3] {
        &mut self.0
    }
}

impl From<ColorRgb> for [f32; 3] {
    fn from(value: ColorRgb) -> Self {
        value.0
    }
}

impl std::ops::AddAssign for ColorRgb {
    fn add_assign(&mut self, rhs: Self) {
        self.0[0] += rhs.0[0];
        self.0[1] += rhs.0[1];
        self.0[2] += rhs.0[2];
    }
}

impl std::ops::Mul for ColorRgb {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self([
            self.0[0] * rhs.0[0],
            self.0[1] * rhs.0[1],
            self.0[2] * rhs.0[2],
        ])
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

impl std::ops::Mul<ColorRgb> for Mat3 {
    type Output = ColorRgb;

    fn mul(self, rhs: ColorRgb) -> Self::Output {
        let rhs = vector![rhs.red(), rhs.green(), rhs.blue()];
        let rhs = self * rhs;
        ColorRgb([rhs.x, rhs.y, rhs.z])
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
        if let Some(p) = f.precision() {
            write!(
                f,
                "{:.*},{:.*},{:.*}",
                p,
                self.red(),
                p,
                self.green(),
                p,
                self.blue(),
            )
        } else {
            write!(f, "{},{},{}", self.red(), self.green(), self.blue())
        }
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
// Exposure
//

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exposure {
    stops: f32,
    exposure: f32,
}

impl Default for Exposure {
    fn default() -> Self {
        Self::new(4.0)
    }
}

impl Exposure {
    pub fn new(stops: f32) -> Self {
        Self {
            stops,
            exposure: Self::precalculate_exposure(stops),
        }
    }

    pub fn expose(self, color: ColorRgb) -> ColorRgb {
        color * self.exposure
    }

    pub fn gui(&mut self, ui: &imgui::Ui) {
        if ui.slider("Exposure", 0.0, 16.0, &mut self.stops) {
            self.exposure = Self::precalculate_exposure(self.stops);
        }
    }

    fn precalculate_exposure(stops: f32) -> f32 {
        1.0 / 2.0_f32.powf(stops)
    }
}

impl std::fmt::Display for Exposure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(precision) = f.precision() {
            write!(f, "{:.precision$}", self.stops)
        } else {
            write!(f, "{}", self.stops)
        }
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
// Spherical coordinate systems
//

#[derive(Clone, Copy, Debug)]
pub struct Spherical {
    angle_xz: f32, // pbrt:phi, azimuthal, [-pi,pi]
    angle_y: f32,  // pbrt:theta, polar, [0,pi]
}

#[derive(Clone, Copy, Debug)]
pub struct NormalizedSpherical {
    angle_xz: f32, // pbrt:phi, azimuthal, [0,1]
    angle_y: f32,  // pbrt:theta, polar, [0,1]
}

impl Spherical {
    #[allow(dead_code)]
    pub fn new(angle_xz: f32, angle_y: f32) -> Self {
        assert!((-PI..=PI).contains(&angle_xz));
        assert!((0.0..=PI).contains(&angle_y));
        Self { angle_xz, angle_y }
    }

    #[allow(dead_code)]
    pub fn angle_xz(self) -> f32 {
        self.angle_xz
    }

    #[allow(dead_code)]
    pub fn angle_y(self) -> f32 {
        self.angle_y
    }

    pub fn to_cartesian(self) -> Normal {
        normal!(
            self.angle_y.sin() * self.angle_xz.cos(),
            self.angle_y.cos(),
            self.angle_y.sin() * self.angle_xz.sin()
        )
    }

    pub fn from_cartesian(c: Normal) -> Self {
        let angle_xz = c.z.atan2(c.x);
        let angle_y = c.y.acos();
        assert!((-PI..=PI).contains(&angle_xz));
        assert!((0.0..=PI).contains(&angle_y));
        Self { angle_xz, angle_y }
    }

    pub fn normalized(self) -> NormalizedSpherical {
        NormalizedSpherical {
            angle_xz: (self.angle_xz + PI) / TAU,
            angle_y: self.angle_y / PI,
        }
    }

    pub fn from_normalized(norm: NormalizedSpherical) -> Self {
        Self {
            angle_xz: (norm.angle_xz - 0.5) * TAU,
            angle_y: norm.angle_y * 0.5 * PI,
        }
    }
}

impl NormalizedSpherical {
    pub fn new(angle_xz: f32, angle_y: f32) -> Self {
        assert!((0.0..=1.0).contains(&angle_xz));
        assert!((0.0..=1.0).contains(&angle_y));
        Self { angle_xz, angle_y }
    }

    pub fn angle_xz(self) -> f32 {
        self.angle_xz
    }

    pub fn angle_y(self) -> f32 {
        self.angle_y
    }
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
    #[allow(dead_code)]
    pub fn center(&self) -> Point3 {
        self.center
    }

    #[inline]
    #[allow(dead_code)]
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
// Stats
//

#[derive(Clone, Copy, Debug)]
pub struct ScalarStats {
    min: f32,
    max: f32,
    sum: f32,
    count: f32,
    avg: f32,
}

impl ScalarStats {
    pub fn new() -> Self {
        Self {
            min: f32::MAX,
            max: -f32::MAX,
            sum: 0.0,
            count: 0.0,
            avg: 0.0,
        }
    }

    pub fn push(&mut self, v: f32) {
        self.min = self.min.min(v);
        self.max = self.max.max(v);
        self.sum += v;
        self.count += 1.0;
        self.avg = self.sum / self.count;
    }

    pub fn min(&self) -> f32 {
        self.min
    }

    pub fn max(&self) -> f32 {
        self.max
    }

    #[allow(dead_code)]
    pub fn sum(&self) -> f32 {
        self.sum
    }

    #[allow(dead_code)]
    pub fn count(&self) -> f32 {
        self.count
    }

    #[allow(dead_code)]
    pub fn avg(&self) -> f32 {
        self.avg
    }
}

impl std::fmt::Display for ScalarStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "min={}, max={}, sum={}, count={}, avg={}",
            self.min, self.max, self.sum, self.count, self.avg
        )
    }
}

//
// Tests
//

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_spherical() {
        let epsilon = 0.001;
        let samples = 50;
        for angle_xz in 0..samples {
            let angle_xz = (angle_xz as f32 + 0.5) / samples as f32;
            let angle_xz = (angle_xz - 0.5) * TAU;

            let angle_y = PI / 2.0;
            let src = Spherical::new(angle_xz, angle_y);
            let v = src.to_cartesian();
            let dst = Spherical::from_cartesian(v);
            println!(
                "angle_xz={:.06} angle_y={:.06} => \
                v=({:.06},{:.06},{:.06}) => \
                angle_xz={:.06} angle_y={:.06}",
                src.angle_xz(),
                src.angle_y(),
                v.x,
                v.y,
                v.z,
                dst.angle_xz(),
                dst.angle_y()
            );
            assert_abs_diff_eq!(src.angle_xz(), dst.angle_xz(), epsilon = epsilon);
            assert_abs_diff_eq!(src.angle_y(), dst.angle_y(), epsilon = epsilon);
        }
        println!();
        for angle_y in 0..samples {
            let angle_y = (angle_y as f32 + 0.5) / samples as f32;
            let angle_y = angle_y * PI;

            let angle_xz = 0.0;
            let src = Spherical::new(angle_xz, angle_y);
            let v = src.to_cartesian();
            let dst = Spherical::from_cartesian(v);
            println!(
                "angle_xz={:.06} angle_y={:.06} => \
                v=({:.06},{:.06},{:.06}) => \
                angle_xz={:.06} angle_y={:.06}",
                src.angle_xz(),
                src.angle_y(),
                v.x,
                v.y,
                v.z,
                dst.angle_xz(),
                dst.angle_y()
            );
            assert_abs_diff_eq!(src.angle_xz(), dst.angle_xz(), epsilon = epsilon);
            assert_abs_diff_eq!(src.angle_y(), dst.angle_y(), epsilon = epsilon);
        }
    }

    #[test]
    fn test_scalar_stats() {
        let mut stats = ScalarStats::new();

        stats.push(0.0);
        assert_ulps_eq!(stats.min(), 0.0);
        assert_ulps_eq!(stats.max(), 0.0);
        assert_ulps_eq!(stats.sum(), 0.0);
        assert_ulps_eq!(stats.count(), 1.0);
        assert_ulps_eq!(stats.avg(), 0.0);

        stats.push(10.0);
        assert_ulps_eq!(stats.min(), 0.0);
        assert_ulps_eq!(stats.max(), 10.0);
        assert_ulps_eq!(stats.sum(), 10.0);
        assert_ulps_eq!(stats.count(), 2.0);
        assert_ulps_eq!(stats.avg(), 5.0);
    }
}
