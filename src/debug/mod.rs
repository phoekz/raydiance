use super::*;

mod plot;

const DEFAULT_SAMPLE_COUNT: u32 = 256;
const DEFAULT_SAMPLE_GRID_WIDTH: u32 = 16; // Must be sqrt(DEFAULT_SAMPLE_COUNT).
const DEFAULT_BASE_COLOR: ColorRgb = ColorRgb::WHITE;

const ANIMATION_DELAY_DEN: u16 = 20;
const ANIMATION_DELAY_NUM: u16 = 1;
const ANIMATION_FRAME_COUNT: u32 = 60;

const PLOT_COLOR_INCOMING: ColorRgb = ColorRgb::new(1.0, 0.0, 1.0);
const PLOT_COLOR_SUN: ColorRgb = ColorRgb::new(1.0, 0.0, 1.0);
const PLOT_COLOR_SAMPLE: ColorRgb = ColorRgb::new(1.0, 0.26225, 0.0);
const PLOT_COLOR_TEXT: ColorRgb = ColorRgb::new(1.0, 1.0, 1.0);
const PLOT_COLOR_POS_X: ColorRgb = ColorRgb::new(1.0, 0.0, 0.0);
const PLOT_COLOR_POS_Y: ColorRgb = ColorRgb::new(0.0, 1.0, 0.0);
const PLOT_COLOR_POS_Z: ColorRgb = ColorRgb::new(0.0, 0.0, 1.0);
const PLOT_COLOR_MID_XY: ColorRgb = ColorRgb::new(0.75, 0.75, 0.0);
const PLOT_COLOR_MID_ZY: ColorRgb = ColorRgb::new(0.0, 0.75, 0.75);
const PLOT_IMAGE_SCALE: u32 = 4;

const ANGLE_PLOT_WIDTH: u32 = 100;
const ANGLE_PLOT_HEIGHT: u32 = 25;

const HEMISPHERE_PLOT_WIDTH: u32 = 100;
const HEMISPHERE_PLOT_HEIGHT: u32 = 100;

#[derive(Clone)]
enum SampleSequence {
    Grid(u32),
    Random(UniformSampler),
    Sobol,
}

impl SampleSequence {
    fn name(&self) -> &'static str {
        match self {
            Self::Grid(_) => "grid",
            Self::Random(_) => "random",
            Self::Sobol => "sobol",
        }
    }

    fn sample(&mut self, sample_index: u32) -> (f32, f32) {
        match self {
            Self::Grid(size) => {
                let x = sample_index % *size;
                let y = sample_index / *size;
                (
                    (x as f32 + 0.5) / (*size as f32),
                    (y as f32 + 0.5) / (*size as f32),
                )
            }
            Self::Random(uniform) => (uniform.sample(), uniform.sample()),
            Self::Sobol => (
                sobol_burley::sample(sample_index, 0, 0),
                sobol_burley::sample(sample_index, 1, 0),
            ),
        }
    }
}

//
// BRDF
//

const FLAG_UNIFORM: u32 = 0b1;
const FLAG_COSINE: u32 = 0b10;

#[derive(Clone, Copy)]
enum BrdfComponent {
    R,   // Reflectance
    Pdf, // Probability density function
}

impl BrdfComponent {
    fn name(self) -> &'static str {
        match self {
            Self::R => "r",
            Self::Pdf => "pdf",
        }
    }
}

//
// BRDF visualizations
//

fn brdf_visualizations() -> Result<()> {
    struct Task {
        name: &'static str,
        model: bxdfs::Model,
        comp: BrdfComponent,
        seq: SampleSequence,
        flags: u32,
        incoming: vz::cfg::Value<f32>,
        roughness: vz::cfg::Value<f32>,
        anisotropic: vz::cfg::Value<f32>,
    }

    impl Default for Task {
        fn default() -> Self {
            Self {
                name: "default",
                model: bxdfs::Model::Lambertian,
                comp: BrdfComponent::R,
                seq: SampleSequence::Sobol,
                flags: 0,
                incoming: vz::cfg::Value::Constant(0.25),
                roughness: vz::cfg::Value::Constant(0.25),
                anisotropic: vz::cfg::Value::Constant(0.0),
            }
        }
    }

    // Default work directory.
    let work_dir = work_dir();

    // Default font.
    let font = vz::font::Font::new()?;

    // Sequences.
    let seq_grid = SampleSequence::Grid(DEFAULT_SAMPLE_GRID_WIDTH);
    let seq_rand = SampleSequence::Random(UniformSampler::new());
    let seq_sobol = SampleSequence::Sobol;

    // Task definitions.
    #[allow(clippy::redundant_clone)]
    let tasks = {
        use bxdfs::Model::{CookTorrance, DisneyDiffuse, Lambertian};
        use BrdfComponent::{Pdf, R};

        macro_rules! hemisphere {
            ($name: expr, $flag: expr) => {
                Task {
                    name: concat!("hemisphere-", $name),
                    seq: seq_grid.clone(),
                    flags: $flag,
                    ..Task::default()
                }
            };
        }
        macro_rules! sequences {
            ($name: expr, $seq: expr) => {
                Task {
                    name: concat!("sequences-", $name),
                    seq: $seq.clone(),
                    ..Task::default()
                }
            };
        }
        macro_rules! lambertian {
            ($name: expr, $comp: ident, $key: ident, $value: expr) => {
                Task {
                    name: concat!("lambertian-", $name),
                    model: Lambertian,
                    comp: $comp,
                    seq: seq_sobol.clone(),
                    $key: $value,
                    ..Task::default()
                }
            };
        }
        macro_rules! disney_diffuse {
            ($name: expr, $comp: ident, $key: ident, $value: expr) => {
                Task {
                    name: concat!("disneydiffuse-", $name),
                    model: DisneyDiffuse,
                    comp: $comp,
                    seq: seq_sobol.clone(),
                    $key: $value,
                    ..Task::default()
                }
            };
        }
        macro_rules! cook_torrance {
            ($name: expr, $comp: ident, $key: ident, $value: expr) => {
                Task {
                    name: concat!("cooktorrance-", $name),
                    model: CookTorrance,
                    comp: $comp,
                    seq: seq_sobol.clone(),
                    $key: $value,
                    ..Task::default()
                }
            };
        }

        let unit = vz::cfg::Value::Keyframes(vec![
            vz::cfg::keyframe!(0.0, 0.0, CubicInOut),
            vz::cfg::keyframe!(1.0, 1.0, CubicInOut),
        ]);

        [
            hemisphere!("uniform", FLAG_UNIFORM),
            hemisphere!("cosine", FLAG_COSINE),
            sequences!("grid", seq_grid),
            sequences!("rand", seq_rand),
            sequences!("sobol", seq_sobol),
            lambertian!("roughness-r", R, roughness, unit.clone()),
            lambertian!("roughness-pdf", Pdf, roughness, unit.clone()),
            lambertian!("incoming-r", R, incoming, unit.clone()),
            lambertian!("incoming-pdf", Pdf, incoming, unit.clone()),
            disney_diffuse!("roughness-r", R, roughness, unit.clone()),
            disney_diffuse!("roughness-pdf", Pdf, roughness, unit.clone()),
            disney_diffuse!("incoming-r", R, incoming, unit.clone()),
            disney_diffuse!("incoming-pdf", Pdf, incoming, unit.clone()),
            cook_torrance!("roughness-r", R, roughness, unit.clone()),
            cook_torrance!("roughness-pdf", Pdf, roughness, unit.clone()),
            cook_torrance!("incoming-r", R, incoming, unit.clone()),
            cook_torrance!("incoming-pdf", Pdf, incoming, unit.clone()),
            cook_torrance!("anisotropic-r", R, anisotropic, unit.clone()),
            cook_torrance!("anisotropic-pdf", Pdf, anisotropic, unit.clone()),
        ]
    };

    // Execute tasks.
    let results = tasks
        .into_par_iter()
        .map(|task| {
            let incoming: vz::anim::Value<_> = task.incoming.into();
            let roughness: vz::anim::Value<_> = task.roughness.into();
            let anisotropic: vz::anim::Value<_> = task.anisotropic.into();

            let mut frames = vec![];
            for frame_index in 0..ANIMATION_FRAME_COUNT {
                // Time.
                let time = (frame_index as f32 + 0.5) / ANIMATION_FRAME_COUNT as f32;

                // Tween.
                let incoming = incoming.value(time);
                let roughness = roughness.value(time);
                let anisotropic = anisotropic.value(time);

                // Incoming vector.
                let incoming_angle_theta = incoming * PI;
                let incoming = bxdfs::LocalVector(
                    vector![
                        f32::cos(incoming_angle_theta),
                        f32::sin(incoming_angle_theta),
                        0.0
                    ]
                    .normalize(),
                );

                // Hemisphere sampler.
                let hemisphere = if task.flags & FLAG_UNIFORM > 0 {
                    HemisphereSampler::Uniform
                } else {
                    HemisphereSampler::Cosine
                };

                // Brdf.
                let brdf: Box<dyn bxdfs::Bxdf> = match task.model {
                    bxdfs::Model::Lambertian => {
                        Box::new(bxdfs::Lambertian::new(&bxdfs::LambertianParams {
                            hemisphere,
                            base_color: DEFAULT_BASE_COLOR,
                        }))
                    }
                    bxdfs::Model::DisneyDiffuse => {
                        Box::new(bxdfs::DisneyDiffuse::new(&bxdfs::DisneyDiffuseParams {
                            hemisphere,
                            base_color: DEFAULT_BASE_COLOR,
                            roughness,
                        }))
                    }
                    bxdfs::Model::CookTorrance => {
                        Box::new(bxdfs::CookTorrance::new(&bxdfs::CookTorranceParams {
                            base_color: DEFAULT_BASE_COLOR,
                            metallic: 1.0,
                            specular: 0.5,
                            specular_tint: 0.0,
                            roughness,
                            anisotropic,
                        }))
                    }
                };

                // Plot.
                let mut plot = plot::Plot::new(|wo| match task.comp {
                    BrdfComponent::R => brdf.eval(&wo, &incoming),
                    BrdfComponent::Pdf => {
                        let pdf = brdf.pdf(&wo, &incoming);
                        ColorRgb::new(pdf, pdf, pdf)
                    }
                });

                // Samples.
                let mut inside_hemisphere = 0;
                let mut sequence = task.seq.clone();
                (0..DEFAULT_SAMPLE_COUNT)
                    .into_iter()
                    .for_each(|sample_index| {
                        let uniform = sequence.sample(sample_index);
                        let vector = match brdf.sample(&incoming, uniform) {
                            Some(sample) => {
                                inside_hemisphere += 1;
                                sample.wi()
                            }
                            None => bxdfs::LocalVector(Y_AXIS),
                        };
                        plot.draw_vector(vector, PLOT_COLOR_SAMPLE);
                    });
                plot.draw_debug_vectors();
                plot.draw_vector(incoming, PLOT_COLOR_INCOMING);

                // Annotate.
                let text_box = vz::annotation::TextBox::new()
                    .line([("name", task.name)])
                    .line([
                        ("model", task.model.name()),
                        ("comp", task.comp.name()),
                        ("seq", task.seq.name()),
                    ])
                    .line([
                        ("roughness", format!("{roughness:.02}")),
                        (
                            "wi",
                            format!("{:.02} deg", incoming_angle_theta.to_degrees()),
                        ),
                        ("anisotropic", format!("{anisotropic:.02}")),
                    ])
                    .line([
                        (
                            "inside",
                            format!("{inside_hemisphere}/{DEFAULT_SAMPLE_COUNT}"),
                        ),
                        ("min", format!("{:.02}", plot.intensities().min())),
                        ("max", format!("{:.02}", plot.intensities().max())),
                    ])
                    .line([
                        ("time", format!("{time:.02}")),
                        ("frame", format!("{frame_index}/{ANIMATION_FRAME_COUNT}")),
                    ])
                    .build();
                let mut image = plot.into_image();
                image.draw_text(&font, PLOT_COLOR_TEXT, &text_box);

                // Push.
                frames.push(image);
            }

            // Boomerang.
            let frames = vz::video::create_boomerang(frames);

            // Render animation.
            let task_name = task.name.to_string();
            let file_name = format!("brdf-{task_name}.apng");
            let path = work_dir.join(&file_name);
            let anim_params = vz::video::Params {
                delay_num: ANIMATION_DELAY_NUM,
                delay_den: ANIMATION_DELAY_DEN,
            };
            vz::video::render(&anim_params, path, frames)?;

            Ok::<_, anyhow::Error>((task_name, file_name))
        })
        .collect::<Vec<_>>();

    // Website.
    {
        use std::io::Write;
        let mut page = vz::page::Builder::new("brdf");
        for result in results {
            let (task_name, file_name) = result?;
            page.push_card(task_name, file_name);
        }
        let file_name = "brdf.html";
        let path = work_dir.join(file_name);
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);
        write!(&mut writer, "{}", page.build()?)?;
        info!("Wrote to {}", path.display());
        info!("Open with http://127.0.0.1:5500/work/{file_name}");
    }

    Ok(())
}

fn sky_model_visualizations() -> Result<()> {
    use cpupt::sky;

    struct Task {
        name: &'static str,
        elevation: vz::cfg::Value<f32>,
        azimuth: vz::cfg::Value<f32>,
        turbidity: vz::cfg::Value<f32>,
        albedo: vz::cfg::Value<ColorRgb>,
    }

    impl Default for Task {
        fn default() -> Self {
            let params = sky::ext::StateExtParams::default();
            let elevation = vz::cfg::Value::Constant(params.elevation);
            let azimuth = vz::cfg::Value::Constant(params.azimuth);
            let turbidity = vz::cfg::Value::Constant(params.turbidity);
            let albedo = vz::cfg::Value::Constant(params.albedo);
            Self {
                name: "default",
                elevation,
                azimuth,
                turbidity,
                albedo,
            }
        }
    }

    let tasks = [
        Task {
            name: "elevation",
            elevation: vz::cfg::Value::Keyframes(vec![
                vz::cfg::keyframe!(0.0, 0.0, CubicInOut),
                vz::cfg::keyframe!(1.0, 90.0_f32.to_radians(), CubicInOut),
            ]),
            ..Task::default()
        },
        Task {
            name: "azimuth",
            azimuth: vz::cfg::Value::Keyframes(vec![
                vz::cfg::keyframe!(0.0, 0.0, CubicInOut),
                vz::cfg::keyframe!(1.0, 360.0_f32.to_radians(), CubicInOut),
            ]),
            ..Task::default()
        },
        Task {
            name: "turbidity",
            turbidity: vz::cfg::Value::Keyframes(vec![
                vz::cfg::keyframe!(0.0, 1.0, CubicInOut),
                vz::cfg::keyframe!(1.0, 10.0, CubicInOut),
            ]),
            ..Task::default()
        },
        Task {
            name: "albedo",
            albedo: vz::cfg::Value::Keyframes(vec![
                vz::cfg::keyframe!(0.0, ColorRgb::BLACK, CubicInOut),
                vz::cfg::keyframe!(1.0, ColorRgb::new(1.0, 0.26225, 0.0), CubicInOut),
            ]),
            ..Task::default()
        },
    ];

    // Default work directory.
    let work_dir = work_dir();

    // Default font.
    let font = vz::font::Font::new()?;

    // Default exposure.
    let exposure = Exposure::default();

    // Execute tasks.
    let results = tasks
        .into_par_iter()
        .map(|task| {
            let elevation: vz::anim::Value<_> = task.elevation.into();
            let azimuth: vz::anim::Value<_> = task.azimuth.into();
            let turbidity: vz::anim::Value<_> = task.turbidity.into();
            let albedo: vz::anim::Value<_> = task.albedo.into();

            let mut frames = vec![];
            for frame_index in 0..ANIMATION_FRAME_COUNT {
                // Time.
                let time = (frame_index as f32 + 0.5) / ANIMATION_FRAME_COUNT as f32;

                // Tween.
                let elevation = elevation.value(time);
                let azimuth = azimuth.value(time);
                let turbidity = turbidity.value(time);
                let albedo = albedo.value(time);

                // Create sky model.
                let sky = sky::ext::StateExt::new(&sky::ext::StateExtParams {
                    elevation,
                    azimuth,
                    turbidity,
                    albedo,
                })?;

                // Plot.
                let mut plot = plot::Plot::new(|wo| {
                    let wo = normal!(wo.0);
                    exposure.expose(sky.radiance(&wo)).tonemap()
                });
                plot.draw_debug_vectors();
                plot.draw_vector(bxdfs::LocalVector(*sky.sun_dir()), PLOT_COLOR_SUN);

                // Annotate.
                let text_box = vz::annotation::TextBox::new()
                    .line([("name", task.name)])
                    .line([
                        ("elevation", format!("{:.02}°", elevation.to_degrees())),
                        ("azimuth", format!("{:.02}°", azimuth.to_degrees())),
                    ])
                    .line([
                        ("turbidity", format!("{turbidity:.02}")),
                        ("albedo", format!("{albedo:.02}")),
                        ("exposure", format!("{exposure:.02}")),
                    ])
                    .line([
                        ("min", format!("{:.02}", plot.intensities().min())),
                        ("max", format!("{:.02}", plot.intensities().max())),
                    ])
                    .line([
                        ("time", format!("{time:.02}")),
                        ("frame", format!("{frame_index}/{ANIMATION_FRAME_COUNT}")),
                    ])
                    .build();
                let mut image = plot.into_image();
                image.draw_text(&font, PLOT_COLOR_TEXT, &text_box);

                // Push.
                frames.push(image);
            }

            // Boomerang.
            let frames = vz::video::create_boomerang(frames);

            // Render animations.
            let file_name = format!("sky-{}.apng", task.name);
            let path = work_dir.join(&file_name);
            let anim_params = vz::video::Params {
                delay_num: ANIMATION_DELAY_NUM,
                delay_den: ANIMATION_DELAY_DEN,
            };
            vz::video::render(&anim_params, path, frames)?;

            Ok::<_, anyhow::Error>((task.name, file_name))
        })
        .collect::<Vec<_>>();

    // Website.
    {
        use std::io::Write;
        let mut page = vz::page::Builder::new("sky");
        for result in results {
            let (task_name, file_name) = result?;
            page.push_card(task_name, file_name);
        }
        let file_name = "sky.html";
        let path = work_dir.join(file_name);
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);
        write!(&mut writer, "{}", page.build()?)?;
        info!("Wrote to {}", path.display());
        info!("Open with http://127.0.0.1:5500/work/{file_name}");
    }

    Ok(())
}

//
// Runner
//

pub fn run() -> Result<()> {
    brdf_visualizations()?;
    sky_model_visualizations()?;
    Ok(())
}
