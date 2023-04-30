use super::*;

#[derive(Clone)]
pub struct Rgb {
    buffer: imagelib::RgbImage,
}

impl Rgb {
    #[inline]
    #[must_use]
    pub fn new(size: (u32, u32)) -> Self {
        Self {
            buffer: imagelib::RgbImage::new(size.0, size.1),
        }
    }

    pub fn from_colors(colors: &[ColorRgb], size: (u32, u32)) -> Self {
        assert!(colors.len() == (size.0 * size.1) as usize);
        let mut image = Self::new(size);
        colors
            .iter()
            .zip(image.buffer.pixels_mut())
            .for_each(|(src, dst)| {
                *dst = imagelib::Rgb(src.clamp().into_srgb8());
            });
        image
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.buffer.width()
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.buffer.height()
    }

    #[inline]
    pub fn put_pixel(&mut self, x: u32, y: u32, color: ColorRgb) {
        let pixel = imagelib::Rgb(color.into_srgb8());
        self.buffer.put_pixel(x, y, pixel);
    }

    #[inline]
    pub fn save(&self, path: &Path) -> Result<()> {
        self.buffer.save(path)?;
        Ok(())
    }

    #[inline]
    pub fn into_dynamic(self) -> imagelib::DynamicImage {
        self.buffer.into()
    }

    pub fn draw_text(&mut self, font: &font::Font<'_>, color: ColorRgb, text: &str) {
        use imagelib::GenericImage;
        use imageproc::drawing::draw_text_mut;

        // Init color.
        let color = imagelib::Rgb(color.into_srgb8());

        // Init font metrics.
        let scale = rusttype::Scale { x: 16.0, y: 16.0 };
        let line_height = font.line_height(scale);
        let line_count = text.lines().count() as u32;
        let text_height = line_count * line_height;

        // How much should the image be expanded to fit all text?
        let image_height = self.height() + (line_count + 1) * line_height;

        // Align up to an even multiple to make certain video codecs happy.
        let height_alignment = 4;
        let image_height = (image_height + (height_alignment - 1)) & !(height_alignment - 1);

        // Margin.
        let margin = line_height / 2;

        // Expand canvas to accomodate text.
        let mut expanded = imagelib::RgbImage::new(self.width(), image_height);
        let bg_color = imagelib::Rgb(ColorRgb::BLACK.into_srgb8());
        expanded.pixels_mut().for_each(|pixel| {
            *pixel = bg_color;
        });

        // Copy original image onto expanded canvas, replace existing image.
        expanded
            .copy_from(&self.buffer, 0, 0)
            .expect("Failed to copy image into expanded frame");
        self.buffer = expanded;

        // Draw text.
        let mut y_offset = text_height;
        for text_line in text.lines() {
            let x = margin as i32;
            let y = (self.buffer.height() - y_offset - margin) as i32;
            draw_text_mut(&mut self.buffer, color, x, y, scale, font.raw(), text_line);
            y_offset -= line_height;
        }
    }
}

impl From<imagelib::RgbImage> for Rgb {
    fn from(buffer: imagelib::RgbImage) -> Self {
        Self { buffer }
    }
}

impl From<imagelib::RgbaImage> for Rgb {
    fn from(buffer: imagelib::RgbaImage) -> Self {
        let buffer = imagelib::DynamicImage::from(buffer);
        let buffer = buffer.into_rgb8();
        Self { buffer }
    }
}

#[allow(dead_code)]
fn create_checkerboard_texture() {
    let w = 32;
    let h = 32;
    let mut img = vec![];
    let b = 0.5;
    for y in 0..h {
        for x in 0..w {
            if (x + y) % 2 == 0 {
                img.push(ColorRgb::new(b, b, b));
            } else {
                img.push(ColorRgb::new(1.0, 1.0, 1.0));
            }
        }
    }
    let img = Rgb::from_colors(&img, (w, h));
    img.save(&PathBuf::from("checkerboard.png")).unwrap();
}
