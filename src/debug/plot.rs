use super::*;

pub struct SurfaceInteraction {
    incoming: bxdfs::LocalVector,
    outgoing: bxdfs::LocalVector,
}

impl SurfaceInteraction {
    pub fn incoming(&self) -> &bxdfs::LocalVector {
        &self.incoming
    }

    pub fn outgoing(&self) -> &bxdfs::LocalVector {
        &self.outgoing
    }
}

#[derive(Clone)]
pub struct Plot {
    angle: frame::Angle,
    hemisphere: frame::Hemisphere,
    intensity: scalar::Range,
    incoming: bxdfs::LocalVector,
}

pub fn new<EvalFn>(incoming: bxdfs::LocalVector, eval_fn: EvalFn) -> Plot
where
    EvalFn: Fn(SurfaceInteraction) -> ColorRgb,
{
    let (angle, intensity) = {
        let size = (ANGLE_PLOT_WIDTH, ANGLE_PLOT_HEIGHT);
        let mut image: frame::Angle = frame::Frame::new(size).into();
        let mut intensity_range = scalar::Range::default();
        for pixel_y in 0..size.1 {
            for pixel_x in 0..size.0 {
                let outgoing = image
                    .vector_from_pixel(pixel_x, pixel_y)
                    .expect("This should never fail");
                let reflectance = eval_fn(SurfaceInteraction { incoming, outgoing });
                let intensity =
                    (reflectance.red() + reflectance.green() + reflectance.blue()) / 3.0;
                intensity_range.add(intensity);
                let intensity = intensity.clamp(0.0, 1.0);
                image.draw_intensity(pixel_x, pixel_y, intensity);
            }
        }
        (image, intensity_range)
    };

    let hemisphere = {
        let size = (HEMISPHERE_PLOT_WIDTH, HEMISPHERE_PLOT_HEIGHT);
        let mut image: frame::Hemisphere = frame::Frame::new(size).into();
        for pixel_y in 0..size.1 {
            for pixel_x in 0..size.0 {
                if let Some(outgoing) = image.vector_from_pixel(pixel_x, pixel_y) {
                    let reflectance = eval_fn(SurfaceInteraction { incoming, outgoing });
                    let intensity =
                        (reflectance.red() + reflectance.green() + reflectance.blue()) / 3.0;
                    let intensity = intensity.clamp(0.0, 1.0);
                    image.draw_intensity(pixel_x, pixel_y, intensity);
                } else {
                    image.draw_intensity(pixel_x, pixel_y, 0.0);
                }
            }
        }
        image
    };

    Plot {
        angle,
        hemisphere,
        intensity,
        incoming,
    }
}

impl Plot {
    pub fn sample_f<Samples>(&mut self, samples: Samples)
    where
        Samples: Iterator<Item = bxdfs::LocalVector>,
    {
        for incoming in samples {
            self.angle.draw_vector(incoming, PLOT_COLOR_SAMPLE);
            self.hemisphere.draw_vector(incoming, PLOT_COLOR_SAMPLE);
        }
    }

    pub fn into_images(self) -> (frame::Frame, frame::Frame) {
        let mut angle = self.angle;
        let mut hemisphere = self.hemisphere;
        angle.draw_debug_vectors(self.incoming);
        hemisphere.draw_debug_vectors(self.incoming);
        (
            Self::write_image(angle.into_inner()),
            Self::write_image(hemisphere.into_inner()),
        )
    }

    pub fn intensity(&self) -> scalar::Range {
        self.intensity
    }

    fn write_image(image: frame::Frame) -> frame::Frame {
        use image::GenericImage;

        // The plots are generated upside down, flip vertically.
        let image = image.into_dynamic();
        let image = image.flipv();

        // Resize to make sample points larger.
        // Todo: we should just draw larger points in the first place.
        let image = image.resize_exact(
            PLOT_IMAGE_SCALE * image.width(),
            PLOT_IMAGE_SCALE * image.height(),
            image::imageops::Nearest,
        );

        // Create expanded canvas for text.
        let mut expanded = image::RgbImage::new(image.width(), image.height() + PLOT_IMAGE_BORDER);
        let bg_color = image::Rgb(PLOT_COLOR_BACKGROUND.to_srgb_bytes());
        expanded.pixels_mut().for_each(|pixel| {
            *pixel = bg_color;
        });

        // Copy original image onto expanded canvas.
        let image = image.into_rgb8();
        expanded
            .copy_from(&image, 0, 0)
            .expect("Failed to copy image into expanded frame");
        expanded.into()
    }
}
