use std::{fmt, ops};

use super::*;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ColorRgb([f32; 3]);

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ColorRgba([f32; 4]);

impl ColorRgb {
    pub const BLACK: Self = Self::new(0.0, 0.0, 0.0);
    pub const WHITE: Self = Self::new(1.0, 1.0, 1.0);

    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self([r, g, b])
    }

    pub const fn r(&self) -> f32 {
        self.0[0]
    }

    pub const fn g(&self) -> f32 {
        self.0[1]
    }

    pub const fn b(&self) -> f32 {
        self.0[2]
    }

    pub fn r_mut(&mut self) -> &mut f32 {
        &mut self.0[0]
    }

    pub fn g_mut(&mut self) -> &mut f32 {
        &mut self.0[1]
    }

    pub fn b_mut(&mut self) -> &mut f32 {
        &mut self.0[2]
    }

    pub fn as_mut(&mut self) -> &mut [f32; 3] {
        &mut self.0
    }

    pub fn is_finite(&self) -> bool {
        self.r().is_finite() && self.g().is_finite() && self.b().is_finite()
    }

    pub fn clamp(self) -> Self {
        Self::new(
            self.r().clamp(0.0, 1.0),
            self.g().clamp(0.0, 1.0),
            self.b().clamp(0.0, 1.0),
        )
    }

    pub fn luminance(self) -> f32 {
        // "3.2: Derivation of luminance signal"
        // https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.709-6-201506-I!!PDF-E.pdf
        // https://en.wikipedia.org/wiki/Relative_luminance
        0.2126 * self.r() + 0.7152 * self.g() + 0.0722 * self.b()
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
        Self::new(aces(self.r()), aces(self.g()), aces(self.b()))
    }

    pub fn into_srgb8(self) -> [u8; 3] {
        use palette::{LinSrgb, Srgb};
        assert!((0.0..=1.0).contains(&self.r()));
        assert!((0.0..=1.0).contains(&self.g()));
        assert!((0.0..=1.0).contains(&self.b()));
        let linear = LinSrgb::<f32>::new(self.r(), self.g(), self.b());
        let srgb = Srgb::<f32>::from_linear(linear);
        srgb.into_format().into()
    }
}

impl From<ColorRgb> for [f32; 3] {
    fn from(value: ColorRgb) -> Self {
        value.0
    }
}

impl ops::Add for ColorRgb {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.r() + rhs.r(), self.g() + rhs.g(), self.b() + rhs.b())
    }
}

impl ops::AddAssign for ColorRgb {
    fn add_assign(&mut self, rhs: Self) {
        *self.r_mut() += rhs.r();
        *self.g_mut() += rhs.g();
        *self.b_mut() += rhs.b();
    }
}

impl ops::Mul for ColorRgb {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.r() * rhs.r(), self.g() * rhs.g(), self.b() * rhs.b())
    }
}

impl ops::Mul<f32> for ColorRgb {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.r() * rhs, self.g() * rhs, self.b() * rhs)
    }
}

impl ops::Mul<ColorRgb> for f32 {
    type Output = ColorRgb;

    fn mul(self, rhs: ColorRgb) -> Self::Output {
        ColorRgb::new(self * rhs.r(), self * rhs.g(), self * rhs.b())
    }
}

impl ops::MulAssign for ColorRgb {
    fn mul_assign(&mut self, rhs: Self) {
        *self.r_mut() *= rhs.r();
        *self.g_mut() *= rhs.g();
        *self.b_mut() *= rhs.b();
    }
}

impl ops::Div<f32> for ColorRgb {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.r() / rhs, self.g() / rhs, self.b() / rhs)
    }
}

impl fmt::Display for ColorRgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(precision) = f.precision() {
            write!(
                f,
                "{:.precision$},{:.precision$},{:.precision$}",
                self.r(),
                self.g(),
                self.b(),
            )
        } else {
            write!(f, "{},{},{}", self.r(), self.g(), self.b())
        }
    }
}

impl ColorRgba {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self([r, g, b, a])
    }

    pub const fn r(&self) -> f32 {
        self.0[0]
    }

    pub const fn g(&self) -> f32 {
        self.0[1]
    }

    pub const fn b(&self) -> f32 {
        self.0[2]
    }

    pub const fn a(&self) -> f32 {
        self.0[3]
    }

    pub const fn rgb(&self) -> ColorRgb {
        ColorRgb::new(self.r(), self.g(), self.b())
    }
}

impl fmt::Display for ColorRgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(precision) = f.precision() {
            write!(
                f,
                "{:.precision$},{:.precision$},{:.precision$},{:.precision$}",
                self.r(),
                self.g(),
                self.b(),
                self.a(),
            )
        } else {
            write!(f, "{},{},{},{}", self.r(), self.g(), self.b(), self.a())
        }
    }
}
