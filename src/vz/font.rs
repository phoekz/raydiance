use super::*;

pub struct Font<'a>(rusttype::Font<'a>);

impl Font<'_> {
    pub fn new() -> Result<Self> {
        let font = Vec::from(include_bytes!("../assets/fonts/SourceSansPro-Regular.ttf") as &[u8]);
        let font =
            rusttype::Font::try_from_vec(font).ok_or_else(|| anyhow!("Failed to load font"))?;
        Ok(Self(font))
    }

    pub fn line_height(&self, scale: rusttype::Scale) -> u32 {
        let v_metrics = self.0.v_metrics(scale);
        (v_metrics.ascent - v_metrics.descent) as u32
    }

    pub fn raw(&self) -> &rusttype::Font<'_> {
        &self.0
    }
}
