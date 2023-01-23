use super::*;

pub struct Renderer<'a> {
    font: rusttype::Font<'a>,
}

impl Renderer<'_> {
    pub fn new() -> Result<Self> {
        let font =
            Vec::from(include_bytes!("../../docs/blog/fonts/SourceSansPro-Regular.ttf") as &[u8]);
        let font =
            rusttype::Font::try_from_vec(font).ok_or_else(|| anyhow!("Failed to load font"))?;
        Ok(Self { font })
    }

    pub fn draw(&self, image: &mut img::Frame, text: &str) {
        use imageproc::drawing::draw_text_mut;
        let scale = rusttype::Scale {
            x: PLOT_TEXT_SCALE,
            y: PLOT_TEXT_SCALE,
        };
        let v_metrics = self.font.v_metrics(scale);
        let line_height = (v_metrics.ascent - v_metrics.descent) as i32;
        let line_count = text.lines().count();
        let text_height = line_count as i32 * line_height;
        let image_height = image.height() as i32;
        let mut y_offset = text_height;
        for line in text.lines() {
            let x = PLOT_TEXT_MARGIN;
            let y = image_height - y_offset - PLOT_TEXT_MARGIN;
            draw_text_mut(image, PLOT_COLOR_TEXT, x, y, scale, &self.font, line);
            y_offset -= line_height;
        }
    }
}
