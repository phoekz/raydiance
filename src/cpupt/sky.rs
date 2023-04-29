use super::*;

use hw_skymodel::rgb as HosekWilkie;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyParams {
    pub elevation: f32,
    pub azimuth: f32,
    pub turbidity: f32,
    pub albedo: ColorRgb,
}

impl Default for SkyParams {
    fn default() -> Self {
        Self {
            elevation: 45.0_f32.to_radians(),
            azimuth: 0.0,
            turbidity: 3.0,
            albedo: ColorRgb::WHITE,
        }
    }
}

pub struct SkyState {
    state: HosekWilkie::SkyState,
    sun_dir: Normal,
}

impl SkyState {
    pub fn new(params: &SkyParams) -> Result<Self> {
        // Validate.
        assert!(
            (0.0..=TAU).contains(&params.azimuth),
            "Solar azimuth must be in [0,2pi], got {} instead",
            params.azimuth
        );

        // Init state.
        let state = HosekWilkie::SkyState::new(&HosekWilkie::SkyParams {
            elevation: params.elevation,
            turbidity: params.turbidity,
            albedo: params.albedo.into(),
        })?;

        // Pre-compute sun direction.
        let sun_angle_xz = params.azimuth;
        let sun_angle_y = 0.5 * PI - params.elevation;
        let sun_dir = normal![
            sun_angle_y.sin() * sun_angle_xz.cos(),
            sun_angle_y.cos(),
            sun_angle_y.sin() * sun_angle_xz.sin()
        ];

        Ok(Self { state, sun_dir })
    }

    pub fn sun_dir(&self) -> Normal {
        self.sun_dir
    }

    pub fn radiance(&self, ray_dir: &Normal) -> ColorRgb {
        let theta = ray_dir.y.acos();
        let cos_gamma = ray_dir.dot(&self.sun_dir).clamp(-1.0, 1.0);
        let gamma = cos_gamma.acos();
        ColorRgb::new(
            self.state.radiance(theta, gamma, HosekWilkie::Channel::R),
            self.state.radiance(theta, gamma, HosekWilkie::Channel::G),
            self.state.radiance(theta, gamma, HosekWilkie::Channel::B),
        )
    }
}

impl GuiElement for SkyParams {
    fn gui(&mut self, ui: &imgui::Ui) {
        imgui::AngleSlider::new("Elevation")
            .min_degrees(0.0)
            .max_degrees(90.0)
            .build(ui, &mut self.elevation);
        imgui::AngleSlider::new("Azimuth")
            .min_degrees(0.0)
            .max_degrees(360.0)
            .build(ui, &mut self.azimuth);
        ui.slider("Turbidity", 1.0, 10.0, &mut self.turbidity);
        ui.color_edit3("Albedo", self.albedo.as_mut());
    }
}
