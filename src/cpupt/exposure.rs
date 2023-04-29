use super::*;

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

impl GuiElement for Exposure {
    fn gui(&mut self, ui: &imgui::Ui) {
        if ui.slider("Exposure", 0.0, 16.0, &mut self.stops) {
            self.exposure = Self::precalculate_exposure(self.stops);
        }
    }
}
