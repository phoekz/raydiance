use super::*;

//
// Font
//

pub struct Font<'a>(rusttype::Font<'a>);

impl Font<'_> {
    pub fn new() -> Result<Self> {
        let font =
            Vec::from(include_bytes!("../../docs/blog/fonts/SourceSansPro-Regular.ttf") as &[u8]);
        let font =
            rusttype::Font::try_from_vec(font).ok_or_else(|| anyhow!("Failed to load font"))?;
        Ok(Self(font))
    }
}

//
// Image
//

#[derive(Clone)]
pub struct RgbImage {
    buffer: image::RgbImage,
}

impl RgbImage {
    #[inline]
    #[must_use]
    pub fn new(size: (u32, u32)) -> Self {
        Self {
            buffer: image::RgbImage::new(size.0, size.1),
        }
    }

    pub fn from_colors(colors: &[ColorRgb], size: (u32, u32)) -> Self {
        assert!(colors.len() == (size.0 * size.1) as usize);
        let mut image = Self::new(size);
        colors
            .iter()
            .zip(image.buffer.pixels_mut())
            .for_each(|(src, dst)| {
                *dst = image::Rgb(src.to_srgb_bytes());
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
        let pixel = image::Rgb(color.to_srgb_bytes());
        self.buffer.put_pixel(x, y, pixel);
    }

    #[inline]
    pub fn save(&self, path: &Path) -> Result<()> {
        self.buffer.save(path)?;
        Ok(())
    }

    #[inline]
    pub fn into_dynamic(self) -> image::DynamicImage {
        self.buffer.into()
    }

    pub fn draw_text(
        &mut self,
        color: ColorRgb,
        scale: f32,
        margin: i32,
        font: &Font<'_>,
        text: &str,
    ) {
        use imageproc::drawing::draw_text_mut;
        let color = image::Rgb(color.to_srgb_bytes());
        let scale = rusttype::Scale { x: scale, y: scale };
        let v_metrics = font.0.v_metrics(scale);
        let line_height = (v_metrics.ascent - v_metrics.descent) as i32;
        let line_count = text.lines().count();
        let text_height = line_count as i32 * line_height;
        let image_height = self.height() as i32;
        let mut y_offset = text_height;
        for text_line in text.lines() {
            let x = margin;
            let y = image_height - y_offset - margin;
            draw_text_mut(&mut self.buffer, color, x, y, scale, &font.0, text_line);
            y_offset -= line_height;
        }
    }
}

impl From<image::RgbImage> for RgbImage {
    fn from(buffer: image::RgbImage) -> Self {
        Self { buffer }
    }
}

//
// Animation
//

pub fn create_boomerang(frames: Vec<RgbImage>) -> Vec<RgbImage> {
    frames
        .clone()
        .into_iter()
        .chain(frames.into_iter().rev())
        .collect()
}

pub struct AnimationParams {
    pub delay_num: u16,
    pub delay_den: u16,
}

pub fn animation_render<Frames, ImagePath>(
    params: &AnimationParams,
    path: ImagePath,
    frames: Frames,
) -> Result<()>
where
    ImagePath: AsRef<Path>,
    Frames: IntoIterator<Item = RgbImage>,
{
    let frame_images = frames.into_iter().collect::<Vec<_>>();
    let width = frame_images[0].width();
    let height = frame_images[0].height();
    let num_frames = frame_images.len() as u32;

    let mut writer = BufWriter::new(File::create(path.as_ref())?);
    let config = apng::Config {
        width,
        height,
        num_frames,
        num_plays: 0,
        color: png::ColorType::Rgb,
        depth: png::BitDepth::Eight,
        filter: png::FilterType::NoFilter,
    };
    let mut encoder = apng::Encoder::new(&mut writer, config)?;
    let frame = apng::Frame {
        delay_num: Some(params.delay_num),
        delay_den: Some(params.delay_den),
        ..apng::Frame::default()
    };
    let images = frame_images
        .into_iter()
        .map(|image| {
            apng::load_dynamic_image(image.into_dynamic()).expect("Failed to load image into apng")
        })
        .collect::<Vec<_>>();
    encoder.encode_all(images, Some(&frame))?;
    info!("Wrote to {}", path.as_ref().display());

    Ok(())
}

//
// Misc
//

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
    let img = RgbImage::from_colors(&img, (w, h));
    img.save(&PathBuf::from("checkerboard.png")).unwrap();
}
