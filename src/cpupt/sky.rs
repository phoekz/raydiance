use super::*;

//
// An Analytic Model for Full Spectral Sky-Dome Radiance
// Lukas Hosek & Alexander Wilkie
// Project page: https://cgg.mff.cuni.cz/projects/SkylightModelling/
// License file: sky/hosek-wilkie-license.txt
//

// This is a rewrite of the original Hosek & Wilkie sky model. Only the RGB
// model is supported, and the precision has been reduced to f32. Solar radiance
// functions and alien-like worlds are also removed. The state size has also
// been trimmed down from 1088 to only 120 bytes.

pub enum Channel {
    R = 0,
    G = 1,
    B = 2,
}

#[derive(Clone, Copy, Debug)]
pub struct Params {
    elevation: f32,
    turbidity: f32,
    albedo: [f32; 3],
}

impl Default for Params {
    fn default() -> Self {
        Self {
            elevation: 0.0,
            turbidity: 1.0,
            albedo: [1.0, 1.0, 1.0],
        }
    }
}

impl Params {
    pub fn elevation(self, elevation: f32) -> Self {
        assert!(
            (0.0..=FRAC_PI_2).contains(&elevation),
            "Solar elevation must be in [0,pi/2], got {elevation} instead"
        );
        Self { elevation, ..self }
    }

    pub fn turbidity(self, turbidity: f32) -> Self {
        assert!(
            (1.0..=10.0).contains(&turbidity),
            "Turbidity must be in [1,10], got {turbidity} instead"
        );
        Self { turbidity, ..self }
    }

    pub fn albedo(self, albedo: [f32; 3]) -> Self {
        for (channel, albedo) in albedo.iter().enumerate() {
            assert!(
                (0.0..=1.0).contains(albedo),
                "Albedo ({channel}) must be in [0,1], got {albedo} instead",
            );
        }
        Self { albedo, ..self }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct State {
    params: [f32; 27],
    radiances: [f32; 3],
}

impl State {
    #[inline(never)]
    pub fn new(sky_params: &Params) -> Self {
        // Load datasets.
        macro_rules! dataset {
            ($path:literal) => {{
                // Note: Rust's include_bytes! doesn't guarantee that the byte
                // slice is aligned. That's why we use include_bytes_aligned
                // crate to guarantee 4-byte alignment, so we can cast into
                // f32-slice.
                use include_bytes_aligned::include_bytes_aligned;
                let bytes = include_bytes_aligned!(4, concat!("sky/", $path));
                bytemuck::cast_slice::<_, f32>(bytes)
            }};
        }
        let params_r = dataset!("hosek-wilkie-params-r");
        let params_g = dataset!("hosek-wilkie-params-g");
        let params_b = dataset!("hosek-wilkie-params-b");
        let radiances_r = dataset!("hosek-wilkie-radiances-r");
        let radiances_g = dataset!("hosek-wilkie-radiances-g");
        let radiances_b = dataset!("hosek-wilkie-radiances-b");

        // Init state.
        let mut params = [0.0; 27];
        let mut radiances = [0.0; 3];
        let elevation = sky_params.elevation;
        let turbidity = sky_params.turbidity;
        let albedo = sky_params.albedo;
        let t = (elevation / (0.5 * PI)).powf(1.0 / 3.0);
        init_params(&mut params[..], params_r, turbidity, albedo[0], t);
        init_params(&mut params[9..], params_g, turbidity, albedo[1], t);
        init_params(&mut params[(9 * 2)..], params_b, turbidity, albedo[2], t);
        init_radiances(&mut radiances[0], radiances_r, turbidity, albedo[0], t);
        init_radiances(&mut radiances[1], radiances_g, turbidity, albedo[1], t);
        init_radiances(&mut radiances[2], radiances_b, turbidity, albedo[2], t);

        Self { params, radiances }
    }

    pub fn radiance(&self, theta: f32, gamma: f32, channel: Channel) -> f32 {
        let channel = channel as usize;
        let r = self.radiances[channel];
        let p = &self.params[(9 * channel)..];
        let p0 = p[0];
        let p1 = p[1];
        let p2 = p[2];
        let p3 = p[3];
        let p4 = p[4];
        let p5 = p[5];
        let p6 = p[6];
        let p7 = p[7];
        let p8 = p[8];

        let cos_gamma = gamma.cos();
        let cos_gamma2 = cos_gamma * cos_gamma;
        let cos_theta = theta.cos().abs();

        let exp_m = (p4 * gamma).exp();
        let ray_m = cos_gamma2;
        let mie_m_lhs = 1.0 + cos_gamma2;
        let mie_m_rhs = (1.0 + p8 * p8 - 2.0 * p8 * cos_gamma).powf(1.5);
        let mie_m = mie_m_lhs / mie_m_rhs;
        let zenith = cos_theta.sqrt();
        let radiance_lhs = 1.0 + p0 * (p1 / (cos_theta + 0.01)).exp();
        let radiance_rhs = p2 + p3 * exp_m + p5 * ray_m + p6 * mie_m + p7 * zenith;
        let radiance_dist = radiance_lhs * radiance_rhs;
        r * radiance_dist
    }
}

fn init_params(out_params: &mut [f32], dataset: &[f32], turbidity: f32, albedo: f32, t: f32) {
    let turbidity_int = turbidity.trunc() as usize;
    let turbidity_rem = turbidity.fract();
    let turbidity_min = turbidity_int.saturating_sub(1);
    let turbidity_max = turbidity_int.min(9);
    let p0 = &dataset[(9 * 6 * turbidity_min)..];
    let p1 = &dataset[(9 * 6 * turbidity_max)..];
    let p2 = &dataset[(9 * 6 * 10 + 9 * 6 * turbidity_min)..];
    let p3 = &dataset[(9 * 6 * 10 + 9 * 6 * turbidity_max)..];
    let s0 = (1.0 - albedo) * (1.0 - turbidity_rem);
    let s1 = (1.0 - albedo) * turbidity_rem;
    let s2 = albedo * (1.0 - turbidity_rem);
    let s3 = albedo * turbidity_rem;

    for i in 0..9 {
        out_params[i] += s0 * quintic::<9>(&p0[i..], t);
        out_params[i] += s1 * quintic::<9>(&p1[i..], t);
        out_params[i] += s2 * quintic::<9>(&p2[i..], t);
        out_params[i] += s3 * quintic::<9>(&p3[i..], t);
    }
}

fn init_radiances(out_radiance: &mut f32, dataset: &[f32], turbidity: f32, albedo: f32, t: f32) {
    let turbidity_int = turbidity.trunc() as usize;
    let turbidity_rem = turbidity.fract();
    let turbidity_min = turbidity_int.saturating_sub(1);
    let turbidity_max = turbidity_int.min(9);
    let p0 = &dataset[(6 * turbidity_min)..];
    let p1 = &dataset[(6 * turbidity_max)..];
    let p2 = &dataset[(6 * 10 + 6 * turbidity_min)..];
    let p3 = &dataset[(6 * 10 + 6 * turbidity_max)..];
    let s0 = (1.0 - albedo) * (1.0 - turbidity_rem);
    let s1 = (1.0 - albedo) * turbidity_rem;
    let s2 = albedo * (1.0 - turbidity_rem);
    let s3 = albedo * turbidity_rem;

    *out_radiance += s0 * quintic::<1>(p0, t);
    *out_radiance += s1 * quintic::<1>(p1, t);
    *out_radiance += s2 * quintic::<1>(p2, t);
    *out_radiance += s3 * quintic::<1>(p3, t);
}

fn quintic<const STRIDE: usize>(p: &[f32], t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t2 * t2;
    let t5 = t4 * t;

    let inv_t = 1.0 - t;
    let inv_t2 = inv_t * inv_t;
    let inv_t3 = inv_t2 * inv_t;
    let inv_t4 = inv_t2 * inv_t2;
    let inv_t5 = inv_t4 * inv_t;

    let m0 = p[0] * inv_t5;
    let m1 = p[STRIDE] * 5.0 * inv_t4 * t;
    let m2 = p[2 * STRIDE] * 10.0 * inv_t3 * t2;
    let m3 = p[3 * STRIDE] * 10.0 * inv_t2 * t3;
    let m4 = p[4 * STRIDE] * 5.0 * inv_t * t4;
    let m5 = p[5 * STRIDE] * t5;

    m0 + m1 + m2 + m3 + m4 + m5
}

//
// Raydiance-specific extensions
//

pub mod ext {
    use super::*;

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
        state: State,
        sun_dir: Normal,
    }

    impl StateExt {
        pub fn new(params: &StateExtParams) -> Self {
            // Validate.
            assert!(
                (0.0..=TAU).contains(&params.azimuth),
                "Solar azimuth must be in [0,2pi], got {} instead",
                params.azimuth
            );

            // Init state.
            let state = State::new(
                &super::Params::default()
                    .elevation(params.elevation)
                    .turbidity(params.turbidity)
                    .albedo(params.albedo.into()),
            );

            // Pre-compute sun direction.
            let sun_angle_xz = params.azimuth;
            let sun_angle_y = 0.5 * PI - params.elevation;
            let sun_dir = normal![
                sun_angle_y.sin() * sun_angle_xz.cos(),
                sun_angle_y.cos(),
                sun_angle_y.sin() * sun_angle_xz.sin()
            ];

            Self { state, sun_dir }
        }

        pub fn sun_dir(&self) -> Normal {
            self.sun_dir
        }

        pub fn radiance(&self, ray_dir: &Normal) -> ColorRgb {
            let theta = ray_dir.y.acos();
            let cos_gamma = ray_dir.dot(&self.sun_dir).clamp(-1.0, 1.0);
            let gamma = cos_gamma.acos();
            ColorRgb::new(
                self.state.radiance(theta, gamma, sky::Channel::R),
                self.state.radiance(theta, gamma, sky::Channel::G),
                self.state.radiance(theta, gamma, sky::Channel::B),
            )
        }
    }
}
