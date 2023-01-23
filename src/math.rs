use super::*;

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

pub fn reflect_vector(v: &na::Vector3<f32>, n: &na::Vector3<f32>) -> na::Vector3<f32> {
    (2.0 * v.dot(n) * n - v).normalize()
}

//
// Tests
//

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_ulps_eq;

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
        let v = na::vector![1.0, 1.0, 0.0].normalize();
        let n = na::vector![0.0, 1.0, 0.0];
        let r = reflect_vector(&v, &n);
        assert_ulps_eq!(v.x, -r.x, max_ulps = 1);
        assert_ulps_eq!(v.y, r.y, max_ulps = 1);
        assert_ulps_eq!(v.z, r.z, max_ulps = 1);
    }
}
