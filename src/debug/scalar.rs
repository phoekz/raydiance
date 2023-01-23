#[derive(Clone, Copy)]
pub struct Range {
    min: f32,
    max: f32,
}

impl Range {
    pub fn add(&mut self, x: f32) {
        self.min = self.min.min(x);
        self.max = self.max.max(x);
    }

    pub fn min(self) -> f32 {
        self.min
    }

    pub fn max(self) -> f32 {
        self.max
    }
}

impl Default for Range {
    fn default() -> Self {
        Self {
            min: f32::MAX,
            max: -f32::MAX,
        }
    }
}

impl std::fmt::Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "min={}, max={}", self.min, self.max)
    }
}
