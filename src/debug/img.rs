use super::*;

pub type Frame = image::RgbImage;

pub trait Draw {
    fn into_inner(self) -> Frame;

    fn draw_vector(&mut self, vector: bxdfs::LocalVector, color: image::Rgb<u8>);
    fn draw_intensity(&mut self, pixel_x: u32, pixel_y: u32, intensity: f32);
    fn vector_from_pixel(&mut self, pixel_x: u32, pixel_y: u32) -> Option<bxdfs::LocalVector>;

    fn draw_debug_vectors(&mut self, incoming: bxdfs::LocalVector) {
        // These are drawn in specific order, such that the interesting vectors
        // are draw on top.
        let mid_xy = X_AXIS + Y_AXIS;
        let mid_zy = Z_AXIS + Y_AXIS;
        self.draw_vector(bxdfs::LocalVector(mid_xy), PLOT_COLOR_MID_XY);
        self.draw_vector(bxdfs::LocalVector(mid_zy), PLOT_COLOR_MID_ZY);

        self.draw_vector(bxdfs::LocalVector(X_AXIS), PLOT_COLOR_POS_X);
        self.draw_vector(bxdfs::LocalVector(Y_AXIS), PLOT_COLOR_POS_Y);
        self.draw_vector(bxdfs::LocalVector(Z_AXIS), PLOT_COLOR_POS_Z);

        self.draw_vector(incoming, PLOT_COLOR_INCOMING);
    }
}

#[derive(Clone)]
pub struct Angle(Frame);

impl Draw for Angle {
    fn into_inner(self) -> Frame {
        self.0
    }

    fn draw_vector(&mut self, vector: bxdfs::LocalVector, color: image::Rgb<u8>) {
        let spherical = Spherical::from_cartesian(na::Unit::new_normalize(vector.0)).normalized();
        assert!(
            (0.0..=0.5).contains(&spherical.angle_y()),
            "Vector must be in the hemisphere, angle_xz={}, angle_y={}",
            spherical.angle_xz(),
            spherical.angle_y()
        );
        let angle_y = 2.0 * (0.5 - spherical.angle_y());

        let width = self.0.width();
        let height = self.0.height();
        let pixel_x = ((spherical.angle_xz() * width as f32) as u32).min(width - 1);
        let pixel_y = ((angle_y * height as f32) as u32).min(height - 1);
        self.0.put_pixel(pixel_x, pixel_y, color);
    }

    fn draw_intensity(&mut self, pixel_x: u32, pixel_y: u32, intensity: f32) {
        assert!(
            (0.0..=1.0).contains(&intensity),
            "Intensity must be between 0..1, got {intensity} instead"
        );
        let gray = (255.0 * intensity) as u8;
        self.0
            .put_pixel(pixel_x, pixel_y, image::Rgb([gray, gray, gray]));
    }

    fn vector_from_pixel(&mut self, pixel_x: u32, pixel_y: u32) -> Option<bxdfs::LocalVector> {
        let width = self.0.width();
        let height = self.0.height();
        let norm_x = (pixel_x as f32 + 0.5) / width as f32;
        let norm_y = (pixel_y as f32 + 0.5) / height as f32;
        let norm_y = 1.0 - norm_y;
        assert!((0.0..=1.0).contains(&norm_x));
        assert!((0.0..=1.0).contains(&norm_y));
        let norm = NormalizedSpherical::new(norm_x, norm_y);
        Some(bxdfs::LocalVector(
            *Spherical::from_normalized(norm).to_cartesian(),
        ))
    }
}

impl From<Frame> for Angle {
    fn from(value: Frame) -> Self {
        Self(value)
    }
}

#[derive(Clone)]
pub struct Hemisphere(Frame);

impl Draw for Hemisphere {
    fn into_inner(self) -> Frame {
        self.0
    }

    fn draw_vector(&mut self, vector: bxdfs::LocalVector, color: image::Rgb<u8>) {
        let width = self.0.width();
        let height = self.0.height();
        let vector = vector.0;
        let norm_x = vector.dot(&X_AXIS);
        let norm_z = vector.dot(&Z_AXIS);
        assert!((-1.0..=1.0).contains(&norm_x));
        assert!((-1.0..=1.0).contains(&norm_z));
        let norm_z = -norm_z;
        let pixel_x = (((0.5 * (norm_x + 1.0)) * width as f32) as u32).min(width - 1);
        let pixel_y = (((0.5 * (norm_z + 1.0)) * height as f32) as u32).min(height - 1);
        self.0.put_pixel(pixel_x, pixel_y, color);
    }

    fn draw_intensity(&mut self, pixel_x: u32, pixel_y: u32, intensity: f32) {
        assert!(
            (0.0..=1.0).contains(&intensity),
            "Intensity must be between 0..1, got {intensity} instead"
        );
        let gray = (255.0 * intensity) as u8;
        self.0
            .put_pixel(pixel_x, pixel_y, image::Rgb([gray, gray, gray]));
    }

    fn vector_from_pixel(&mut self, pixel_x: u32, pixel_y: u32) -> Option<bxdfs::LocalVector> {
        let width = self.0.width();
        let height = self.0.height();
        let x = (pixel_x as f32 + 0.5) / width as f32;
        let x = 2.0 * (x - 0.5);
        let z = (pixel_y as f32 + 0.5) / height as f32;
        let z = 2.0 * (z - 0.5);
        let r = na::vector![x, z].norm();
        if r > 1.0 {
            return None;
        }
        let y = r.cos();
        assert!((-1.0..=1.0).contains(&x));
        assert!((-1.0..=1.0).contains(&y));
        assert!((-1.0..=1.0).contains(&z));
        Some(bxdfs::LocalVector(na::vector![x, y, z].normalize()))
    }
}

impl From<Frame> for Hemisphere {
    fn from(value: Frame) -> Self {
        Self(value)
    }
}
