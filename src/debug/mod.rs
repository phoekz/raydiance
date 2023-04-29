use super::*;

use cpupt::bxdfs;

mod plot;

const DEFAULT_SAMPLE_COUNT: u32 = 256;
const DEFAULT_SAMPLE_GRID_WIDTH: u32 = 16; // Must be sqrt(DEFAULT_SAMPLE_COUNT).
const DEFAULT_BASE_COLOR: ColorRgb = ColorRgb::WHITE;
const DEFAULT_ALT_BASE_COLOR: ColorRgb = ColorRgb::new(0.0, 0.42118, 1.0);

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
    Random(cpupt::UniformSampler),
    Sobol,
}

impl SampleSequence {
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

impl std::fmt::Display for SampleSequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Grid(_) => "grid",
                Self::Random(_) => "random",
                Self::Sobol => "sobol",
            }
        )
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

impl std::fmt::Display for BrdfComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                BrdfComponent::R => "r",
                BrdfComponent::Pdf => "pdf",
            }
        )
    }
}

//
// BRDF visualizations
//

fn brdf_visualizations() -> Result<()> {
    struct Task {
        group: &'static str,
        name: &'static str,
        model: bxdfs::Model,
        comp: BrdfComponent,
        seq: SampleSequence,
        flags: u32,
        incoming: vz::cfg::Value<f32>,
        roughness: vz::cfg::Value<f32>,
        base_color: vz::cfg::Value<ColorRgb>,
        anisotropic: vz::cfg::Value<f32>,
        metallic: vz::cfg::Value<f32>,
        specular: vz::cfg::Value<f32>,
        specular_tint: vz::cfg::Value<f32>,
        sheen: vz::cfg::Value<f32>,
        sheen_tint: vz::cfg::Value<f32>,
    }

    impl Default for Task {
        fn default() -> Self {
            use vz::cfg::Value::Constant;
            Self {
                group: "default",
                name: "default",
                model: bxdfs::Model::Lambertian,
                comp: BrdfComponent::R,
                seq: SampleSequence::Sobol,
                flags: 0,
                incoming: Constant(0.25),
                base_color: Constant(DEFAULT_BASE_COLOR),
                roughness: Constant(0.25),
                anisotropic: Constant(0.0),
                metallic: Constant(0.0),
                specular: Constant(0.5),
                specular_tint: Constant(1.0),
                sheen: Constant(0.0),
                sheen_tint: Constant(1.0),
            }
        }
    }

    // Default work directory.
    let work_dir = work_dir();

    // Default font.
    let font = vz::font::Font::new()?;

    // Sequences.
    let seq_grid = SampleSequence::Grid(DEFAULT_SAMPLE_GRID_WIDTH);
    let seq_rand = SampleSequence::Random(cpupt::UniformSampler::new());
    let seq_sobol = SampleSequence::Sobol;

    // Task definitions.
    let tasks = {
        use bxdfs::Model::{DisneyDiffuse, DisneySheen, DisneySpecular, Lambertian};
        use vz::cfg::Value::{Constant, Keyframes};
        use BrdfComponent::{Pdf, R};

        macro_rules! hemisphere {
            ($name: expr, $flag: expr) => {
                vec![Task {
                    group: "hemisphere",
                    name: $name,
                    seq: seq_grid.clone(),
                    flags: $flag,
                    ..Task::default()
                }]
            };
        }
        macro_rules! sequences {
            ($name: expr, $seq: expr) => {
                vec![Task {
                    group: "sequences",
                    name: $name,
                    seq: $seq.clone(),
                    ..Task::default()
                }]
            };
        }
        macro_rules! lambertian {
            ($name: expr, $key: ident, $value: expr) => {
                vec![
                    Task {
                        group: "lambertian",
                        name: concat!($name, "-", "r"),
                        model: Lambertian,
                        comp: R,
                        seq: seq_sobol.clone(),
                        $key: $value.clone(),
                        ..Task::default()
                    },
                    Task {
                        group: "lambertian",
                        name: concat!($name, "-", "pdf"),
                        model: Lambertian,
                        comp: Pdf,
                        seq: seq_sobol.clone(),
                        $key: $value.clone(),
                        ..Task::default()
                    },
                ]
            };
        }
        macro_rules! disney_diffuse {
            ($name: expr, $key: ident, $value: expr) => {
                vec![
                    Task {
                        group: "disney-diffuse",
                        name: concat!($name, "-", "r"),
                        model: DisneyDiffuse,
                        comp: R,
                        seq: seq_sobol.clone(),
                        $key: $value.clone(),
                        ..Task::default()
                    },
                    Task {
                        group: "disney-diffuse",
                        name: concat!($name, "-", "pdf"),
                        model: DisneyDiffuse,
                        comp: Pdf,
                        seq: seq_sobol.clone(),
                        $key: $value.clone(),
                        ..Task::default()
                    },
                ]
            };
        }
        macro_rules! disney_specular {
            ($name: expr, $key: ident, $value: expr) => {
                vec![
                    Task {
                        group: "disney-specular",
                        name: concat!($name, "-", "r"),
                        model: DisneySpecular,
                        comp: R,
                        seq: seq_sobol.clone(),
                        $key: $value.clone(),
                        ..Task::default()
                    },
                    Task {
                        group: "disney-specular",
                        name: concat!($name, "-", "pdf"),
                        model: DisneySpecular,
                        comp: Pdf,
                        seq: seq_sobol.clone(),
                        $key: $value.clone(),
                        ..Task::default()
                    },
                ]
            };
        }
        macro_rules! disney_sheen {
            ($name: expr, $key: ident, $value: expr) => {
                vec![
                    Task {
                        group: "disney-sheen",
                        name: concat!($name, "-", "r"),
                        model: DisneySheen,
                        comp: R,
                        seq: seq_sobol.clone(),
                        $key: $value.clone(),
                        base_color: Constant(DEFAULT_ALT_BASE_COLOR),
                        sheen: Constant(1.0),
                        ..Task::default()
                    },
                    Task {
                        group: "disney-sheen",
                        name: concat!($name, "-", "pdf"),
                        model: DisneySheen,
                        comp: Pdf,
                        seq: seq_sobol.clone(),
                        $key: $value.clone(),
                        base_color: Constant(DEFAULT_ALT_BASE_COLOR),
                        sheen: Constant(1.0),
                        ..Task::default()
                    },
                ]
            };
        }

        let unit = Keyframes(vec![
            vz::cfg::keyframe!(0.0, 0.0, CubicInOut),
            vz::cfg::keyframe!(1.0, 1.0, CubicInOut),
        ]);

        let unit_color = Keyframes(vec![
            vz::cfg::keyframe!(0.0, DEFAULT_BASE_COLOR, CubicInOut),
            vz::cfg::keyframe!(1.0, DEFAULT_ALT_BASE_COLOR, CubicInOut),
        ]);

        vec![
            hemisphere!("uniform", FLAG_UNIFORM),
            hemisphere!("cosine", FLAG_COSINE),
            sequences!("grid", seq_grid),
            sequences!("rand", seq_rand),
            sequences!("sobol", seq_sobol),
            lambertian!("incoming", incoming, unit),
            lambertian!("base_color", base_color, unit_color),
            lambertian!("roughness", roughness, unit),
            disney_diffuse!("incoming", incoming, unit),
            disney_diffuse!("base_color", base_color, unit_color),
            disney_diffuse!("roughness", roughness, unit),
            disney_specular!("incoming", incoming, unit),
            disney_specular!("base_color", base_color, unit_color),
            disney_specular!("roughness", roughness, unit),
            disney_specular!("metallic", metallic, unit),
            disney_specular!("specular", specular, unit),
            disney_specular!("specular_tint", specular_tint, unit),
            disney_specular!("anisotropic", anisotropic, unit),
            disney_sheen!("incoming", incoming, unit),
            disney_sheen!("sheen_tint", sheen_tint, unit),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
    };

    // Execute tasks.
    let results = tasks
        .into_par_iter()
        .map(|task| {
            let base_color: vz::anim::Value<_> = task.base_color.into();
            let incoming: vz::anim::Value<_> = task.incoming.into();
            let roughness: vz::anim::Value<_> = task.roughness.into();
            let anisotropic: vz::anim::Value<_> = task.anisotropic.into();
            let metallic: vz::anim::Value<_> = task.metallic.into();
            let specular: vz::anim::Value<_> = task.specular.into();
            let specular_tint: vz::anim::Value<_> = task.specular_tint.into();
            let sheen: vz::anim::Value<_> = task.sheen.into();
            let sheen_tint: vz::anim::Value<_> = task.sheen_tint.into();

            let mut frames = vec![];
            for frame_index in 0..ANIMATION_FRAME_COUNT {
                // Time.
                let time = (frame_index as f32 + 0.5) / ANIMATION_FRAME_COUNT as f32;

                // Tween.
                let base_color = base_color.value(time);
                let incoming = incoming.value(time);
                let roughness = roughness.value(time);
                let anisotropic = anisotropic.value(time);
                let metallic = metallic.value(time);
                let specular = specular.value(time);
                let specular_tint = specular_tint.value(time);
                let sheen = sheen.value(time);
                let sheen_tint = sheen_tint.value(time);

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
                    cpupt::HemisphereSampler::Uniform
                } else {
                    cpupt::HemisphereSampler::Cosine
                };

                // Brdf.
                let brdf: Box<dyn bxdfs::Bxdf> = match task.model {
                    bxdfs::Model::Lambertian => {
                        Box::new(bxdfs::Lambertian::new(&bxdfs::LambertianParams {
                            hemisphere,
                            base_color,
                        }))
                    }
                    bxdfs::Model::DisneyDiffuse => {
                        Box::new(bxdfs::DisneyDiffuse::new(&bxdfs::DisneyDiffuseParams {
                            hemisphere,
                            base_color,
                            roughness,
                        }))
                    }
                    bxdfs::Model::DisneySpecular => {
                        Box::new(bxdfs::DisneySpecular::new(&bxdfs::DisneySpecularParams {
                            base_color,
                            metallic,
                            specular,
                            specular_tint,
                            roughness,
                            anisotropic,
                        }))
                    }
                    bxdfs::Model::DisneySheen => {
                        Box::new(bxdfs::DisneySheen::new(&bxdfs::DisneySheenParams {
                            hemisphere,
                            base_color,
                            sheen,
                            sheen_tint,
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
                (0..DEFAULT_SAMPLE_COUNT).for_each(|sample_index| {
                    let uniform = sequence.sample(sample_index);
                    let vector = match brdf.sample(&incoming, uniform) {
                        Some(sample) => {
                            inside_hemisphere += 1;
                            sample.wi
                        }
                        None => bxdfs::LocalVector(Y_AXIS),
                    };
                    plot.draw_vector(vector, PLOT_COLOR_SAMPLE);
                });
                plot.draw_debug_vectors();
                plot.draw_vector(incoming, PLOT_COLOR_INCOMING);

                // Annotate.
                let text_box = vz::annotation::TextBox::new()
                    .line([("group", task.group), ("name", task.name)])
                    .line([
                        ("model", format!("{}", task.model)),
                        ("comp", format!("{}", task.comp)),
                        ("seq", format!("{}", task.seq)),
                    ])
                    .line([
                        ("roughness", format!("{roughness:.02}")),
                        ("wi", format!("{:.02}°", incoming_angle_theta.to_degrees())),
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
            let frames = vz::apng::create_boomerang(frames);

            // Render animation.
            let task_group = task.group.to_string();
            let task_name = task.name.to_string();
            let file_name = format!("brdf-{task_group}-{task_name}.apng");
            let path = work_dir.join(&file_name);
            let anim_params = vz::apng::Params {
                delay_num: ANIMATION_DELAY_NUM,
                delay_den: ANIMATION_DELAY_DEN,
            };
            vz::apng::render(&anim_params, path, frames)?;

            Ok::<_, anyhow::Error>((task_group, task_name, file_name))
        })
        .collect::<Vec<_>>();

    // Website.
    {
        use std::io::Write;
        let mut page = vz::page::Builder::new("brdf");
        for result in results {
            let (task_group, task_name, file_name) = result?;
            page.push_card(task_group, task_name, file_name);
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
    struct Task {
        name: &'static str,
        elevation: vz::cfg::Value<f32>,
        azimuth: vz::cfg::Value<f32>,
        turbidity: vz::cfg::Value<f32>,
        albedo: vz::cfg::Value<ColorRgb>,
    }

    impl Default for Task {
        fn default() -> Self {
            let params = cpupt::SkyParams::default();
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
    let exposure = cpupt::Exposure::default();

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
                let sky = cpupt::SkyState::new(&cpupt::SkyParams {
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
            let frames = vz::apng::create_boomerang(frames);

            // Render animations.
            let file_name = format!("sky-{}.apng", task.name);
            let path = work_dir.join(&file_name);
            let anim_params = vz::apng::Params {
                delay_num: ANIMATION_DELAY_NUM,
                delay_den: ANIMATION_DELAY_DEN,
            };
            vz::apng::render(&anim_params, path, frames)?;

            Ok::<_, anyhow::Error>((task.name, file_name))
        })
        .collect::<Vec<_>>();

    // Website.
    {
        use std::io::Write;
        let mut page = vz::page::Builder::new("sky");
        for result in results {
            let (task_name, file_name) = result?;
            page.push_card("sky", task_name, file_name);
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
