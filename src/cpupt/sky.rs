use super::*;

//
// An Analytic Model for Full Spectral Sky-Dome Radiance
// Lukas Hosek & Alexander Wilkie
// Project page: https://cgg.mff.cuni.cz/projects/SkylightModelling/
// License file: sky/hosek-wilkie-license.txt
//

//
// Raydiance-specific extensions
//

pub mod ext {
    use super::*;

    use hw_skymodel::rgb::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct StateExtParams {
        pub elevation: f32,
        pub azimuth: f32,
        pub turbidity: f32,
        pub albedo: ColorRgb,
    }

    impl Default for StateExtParams {
        fn default() -> Self {
            Self {
                elevation: 45.0_f32.to_radians(),
                azimuth: 0.0,
                turbidity: 1.0,
                albedo: ColorRgb::WHITE,
            }
        }
    }

    pub struct StateExt {
        state: SkyState,
        sun_dir: Normal,
    }

    impl StateExt {
        pub fn new(params: &StateExtParams) -> Result<Self> {
            // Validate.
            assert!(
                (0.0..=TAU).contains(&params.azimuth),
                "Solar azimuth must be in [0,2pi], got {} instead",
                params.azimuth
            );

            // Init state.
            let state = SkyState::new(&SkyParams {
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
                self.state.radiance(theta, gamma, Channel::R),
                self.state.radiance(theta, gamma, Channel::G),
                self.state.radiance(theta, gamma, Channel::B),
            )
        }
    }
}
