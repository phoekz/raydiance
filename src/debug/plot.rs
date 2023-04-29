use super::*;

use cpupt::bxdfs;

pub struct Plot {
    image: vz::image::Rgb,
    intensities: ScalarStats,
}

impl Plot {
    pub fn new<EvalFn>(eval_fn: EvalFn) -> Self
    where
        EvalFn: Fn(bxdfs::LocalVector) -> ColorRgb,
    {
        assert_eq!(HEMISPHERE_PLOT_WIDTH, ANGLE_PLOT_WIDTH);
        let width = HEMISPHERE_PLOT_WIDTH;
        let height = HEMISPHERE_PLOT_HEIGHT + ANGLE_PLOT_HEIGHT;
        let mut image = vz::image::Rgb::new((width, height));
        let mut intensities = ScalarStats::new();

        // Hemisphere plot.
        for pixel_y in 0..HEMISPHERE_PLOT_HEIGHT {
            for pixel_x in 0..HEMISPHERE_PLOT_WIDTH {
                if let Some(outgoing) = hemisphere::vector_from_pixel(pixel_x, pixel_y) {
                    let reflectance = eval_fn(outgoing);
                    assert!(reflectance.is_finite());
                    intensities.push(reflectance.r());
                    intensities.push(reflectance.g());
                    intensities.push(reflectance.b());
                    image.put_pixel(pixel_x, ANGLE_PLOT_HEIGHT + pixel_y, reflectance.clamp());
                } else {
                    image.put_pixel(pixel_x, ANGLE_PLOT_HEIGHT + pixel_y, ColorRgb::BLACK);
                }
            }
        }

        // Angle plot.
        for pixel_y in 0..ANGLE_PLOT_HEIGHT {
            for pixel_x in 0..ANGLE_PLOT_WIDTH {
                let outgoing = angle::vector_from_pixel(pixel_x, pixel_y);
                let reflectance = eval_fn(outgoing);
                assert!(reflectance.is_finite());
                intensities.push(reflectance.r());
                intensities.push(reflectance.g());
                intensities.push(reflectance.b());
                image.put_pixel(pixel_x, pixel_y, reflectance.clamp());
            }
        }

        Self { image, intensities }
    }

    pub fn intensities(&self) -> ScalarStats {
        self.intensities
    }

    pub fn draw_vector(&mut self, vector: bxdfs::LocalVector, color: ColorRgb) {
        {
            let width = HEMISPHERE_PLOT_WIDTH;
            let height = HEMISPHERE_PLOT_HEIGHT;
            let vector = vector.0;
            let norm_x = vector.dot(&X_AXIS);
            let norm_z = vector.dot(&Z_AXIS);
            assert!((-1.0..=1.0).contains(&norm_x));
            assert!((-1.0..=1.0).contains(&norm_z));
            let norm_z = -norm_z;
            let pixel_x = (((0.5 * (norm_x + 1.0)) * width as f32) as u32).min(width - 1);
            let pixel_y = (((0.5 * (norm_z + 1.0)) * height as f32) as u32).min(height - 1);
            self.image
                .put_pixel(pixel_x, ANGLE_PLOT_HEIGHT + pixel_y, color);
        }

        {
            let spherical = Spherical::from_cartesian(normal!(vector.0)).normalized();
            assert!(
                (0.0..=0.5).contains(&spherical.angle_y()),
                "Vector must be in the hemisphere, angle_xz={}, angle_y={}",
                spherical.angle_xz(),
                spherical.angle_y()
            );
            let angle_y = 2.0 * (0.5 - spherical.angle_y());

            let width = ANGLE_PLOT_WIDTH;
            let height = ANGLE_PLOT_HEIGHT;
            let pixel_x = ((spherical.angle_xz() * width as f32) as u32).min(width - 1);
            let pixel_y = ((angle_y * height as f32) as u32).min(height - 1);
            self.image.put_pixel(pixel_x, pixel_y, color);
        }
    }

    pub fn draw_debug_vectors(&mut self) {
        let mid_xy = X_AXIS + Y_AXIS;
        let mid_zy = Z_AXIS + Y_AXIS;
        self.draw_vector(bxdfs::LocalVector(mid_xy), PLOT_COLOR_MID_XY);
        self.draw_vector(bxdfs::LocalVector(mid_zy), PLOT_COLOR_MID_ZY);

        self.draw_vector(bxdfs::LocalVector(X_AXIS), PLOT_COLOR_POS_X);
        self.draw_vector(bxdfs::LocalVector(Y_AXIS), PLOT_COLOR_POS_Y);
        self.draw_vector(bxdfs::LocalVector(Z_AXIS), PLOT_COLOR_POS_Z);
    }

    pub fn into_image(self) -> vz::image::Rgb {
        // The plots are generated upside down, flip vertically.
        let image = self.image.into_dynamic();
        let image = image.flipv();

        // Resize to make sample points larger.
        // Todo: we should just draw larger points in the first place.
        let image = image.resize_exact(
            PLOT_IMAGE_SCALE * image.width(),
            PLOT_IMAGE_SCALE * image.height(),
            imagelib::imageops::Nearest,
        );
        image.into_rgb8().into()
    }
}

mod hemisphere {
    use super::*;

    pub fn vector_from_pixel(pixel_x: u32, pixel_y: u32) -> Option<bxdfs::LocalVector> {
        let width = HEMISPHERE_PLOT_WIDTH;
        let height = HEMISPHERE_PLOT_HEIGHT;
        let x = (pixel_x as f32 + 0.5) / width as f32;
        let x = 2.0 * (x - 0.5);
        let z = (pixel_y as f32 + 0.5) / height as f32;
        let z = -2.0 * (z - 0.5);
        let r = vector![x, z].norm();
        if r > 1.0 {
            return None;
        }
        let y = r.cos();
        assert!((-1.0..=1.0).contains(&x));
        assert!((-1.0..=1.0).contains(&y));
        assert!((-1.0..=1.0).contains(&z));
        Some(bxdfs::LocalVector(vector![x, y, z].normalize()))
    }
}

mod angle {
    use super::*;

    pub fn vector_from_pixel(pixel_x: u32, pixel_y: u32) -> bxdfs::LocalVector {
        let width = ANGLE_PLOT_WIDTH;
        let height = ANGLE_PLOT_HEIGHT;
        let norm_x = (pixel_x as f32 + 0.5) / width as f32;
        let norm_y = (pixel_y as f32 + 0.5) / height as f32;
        let norm_y = 1.0 - norm_y;
        assert!((0.0..=1.0).contains(&norm_x));
        assert!((0.0..=1.0).contains(&norm_y));
        let norm = NormalizedSpherical::new(norm_x, norm_y);
        bxdfs::LocalVector(*Spherical::from_normalized(norm).to_cartesian())
    }
}
