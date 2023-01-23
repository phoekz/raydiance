use super::*;

mod animation;
mod img;
mod plot;
mod scalar;
mod spherical;
mod text;

use img::Draw;
use spherical::{NormalizedSpherical, Spherical};

const X_AXIS: na::Vector3<f32> = na::vector![1.0, 0.0, 0.0];
const Y_AXIS: na::Vector3<f32> = na::vector![0.0, 1.0, 0.0];
const Z_AXIS: na::Vector3<f32> = na::vector![0.0, 0.0, 1.0];

const DEFAULT_SAMPLE_COUNT: u32 = 256;
const DEFAULT_SAMPLE_GRID_WIDTH: u32 = 16; // Must be sqrt(DEFAULT_SAMPLE_COUNT).
const DEFAULT_INCOMING: bxdfs::LocalVector =
    bxdfs::LocalVector(na::vector![-0.70710677, 0.70710677, 0.0]);
const DEFAULT_SCALAR: f32 = 1.0 / 4.0;
const DEFAULT_ANISOTROPIC: f32 = 0.0;
const DEFAULT_BASE_COLOR: ColorRgb = ColorRgb::WHITE;

const ANIMATION_DELAY_DEN: u16 = 20;
const ANIMATION_DELAY_NUM: u16 = 1;
const ANIMATION_FRAME_COUNT: u32 = 60;

const PLOT_COLOR_BACKGROUND: image::Rgb<u8> = image::Rgb([0, 0, 0]);
const PLOT_COLOR_INCOMING: image::Rgb<u8> = image::Rgb([255, 0, 255]);
const PLOT_COLOR_SAMPLE: image::Rgb<u8> = image::Rgb([255, 96, 0]);
const PLOT_COLOR_TEXT: image::Rgb<u8> = image::Rgb([255, 255, 255]);
const PLOT_COLOR_POS_X: image::Rgb<u8> = image::Rgb([255, 0, 0]);
const PLOT_COLOR_POS_Y: image::Rgb<u8> = image::Rgb([0, 255, 0]);
const PLOT_COLOR_POS_Z: image::Rgb<u8> = image::Rgb([0, 0, 255]);
const PLOT_COLOR_MID_XY: image::Rgb<u8> = image::Rgb([192, 192, 0]);
const PLOT_COLOR_MID_ZY: image::Rgb<u8> = image::Rgb([0, 192, 192]);
const PLOT_IMAGE_BORDER: u32 = 60;
const PLOT_IMAGE_SCALE: u32 = 4;
const PLOT_TEXT_MARGIN: i32 = 8;
const PLOT_TEXT_SCALE: f32 = 16.0;

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
// BRDF - Selector
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
// BRDF Inputs
//

#[derive(Clone, Copy, Debug)]
struct BrdfInput {
    scalar: f32,
    incoming: bxdfs::LocalVector,
    angle_theta: f32,
    anisotropic: f32,
}

#[derive(Clone, Copy)]
enum BrdfInputParameter<T> {
    Constant(T),
    Interpolated,
}

#[derive(Clone, Copy)]
struct BrdfInputBuilder {
    scalar: BrdfInputParameter<f32>,
    incoming: BrdfInputParameter<(bxdfs::LocalVector, f32)>,
    anisotropic: BrdfInputParameter<f32>,
}

impl Default for BrdfInputBuilder {
    fn default() -> Self {
        Self {
            scalar: BrdfInputParameter::Constant(DEFAULT_SCALAR),
            incoming: BrdfInputParameter::Constant((
                DEFAULT_INCOMING,
                DEFAULT_INCOMING.0.dot(&Y_AXIS),
            )),
            anisotropic: BrdfInputParameter::Constant(DEFAULT_ANISOTROPIC),
        }
    }
}

impl BrdfInputBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn lerp_scalar(mut self) -> Self {
        self.scalar = BrdfInputParameter::Interpolated;
        self
    }

    fn lerp_incoming(mut self) -> Self {
        self.incoming = BrdfInputParameter::Interpolated;
        self
    }

    fn anisotropic(mut self, anisotropic: f32) -> Self {
        self.anisotropic = BrdfInputParameter::Constant(anisotropic);
        self
    }

    fn into_sequence(self) -> Vec<BrdfInput> {
        (0..ANIMATION_FRAME_COUNT)
            .into_iter()
            .map(|frame_index| {
                use easer::functions::*;
                let x = (frame_index as f32 + 0.5) / ANIMATION_FRAME_COUNT as f32;
                let x = Cubic::ease_in_out(x, 0.0, 1.0, 1.0);

                let scalar = match self.scalar {
                    BrdfInputParameter::Constant(c) => c,
                    BrdfInputParameter::Interpolated => x,
                };

                let (incoming, angle_incoming) = match self.incoming {
                    BrdfInputParameter::Constant(c) => c,
                    BrdfInputParameter::Interpolated => {
                        let angle_incoming = x * PI;
                        let incoming = bxdfs::LocalVector(
                            na::vector![
                                f32::cos(angle_incoming - PI),
                                f32::sin(angle_incoming),
                                0.0
                            ]
                            .normalize(),
                        );
                        (incoming, angle_incoming)
                    }
                };

                let anisotropic = match self.anisotropic {
                    BrdfInputParameter::Constant(c) => c,
                    BrdfInputParameter::Interpolated => x,
                };

                BrdfInput {
                    scalar,
                    incoming,
                    angle_theta: angle_incoming,
                    anisotropic,
                }
            })
            .collect()
    }
}

//
// Plot annotation
//

struct PlotHeader<'a> {
    model: &'a str,
    comp: &'a str,
    name: &'a str,
    seq: &'a str,
}

struct PlotAnnotation<'a> {
    header: &'a PlotHeader<'a>,
    scalar: f32,
    incoming: f32,
    intensity: scalar::Range,
    anisotropic: f32,
    inside_hemisphere: u32,
    sample_count: u32,
}

#[derive(Clone, Copy)]
enum PlotType {
    Hemisphere,
    Angle,
}

impl PlotType {
    fn name(self) -> &'static str {
        match self {
            PlotType::Hemisphere => "hemisphere",
            PlotType::Angle => "angle",
        }
    }
}

impl PlotHeader<'_> {
    fn to_filename(&self, plot_type: PlotType) -> String {
        format!(
            "{model}-{comp}-{name}-{seq}-{plot_type}.png",
            model = self.model,
            comp = self.comp,
            name = self.name,
            seq = self.seq,
            plot_type = plot_type.name()
        )
    }
}

impl std::fmt::Display for PlotAnnotation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "model={model}, comp={comp}, name={name}, seq={seq}\n\
            scalar={scalar:.02}, wi={wi:.02} deg, anisotropic={anisotropic:.02}\n\
            inside={inside_hemisphere}/{sample_count}, min={min:.02}, max={max:.02}",
            model = self.header.model,
            comp = self.header.comp,
            name = self.header.name,
            seq = self.header.seq,
            scalar = self.scalar,
            wi = self.incoming.to_degrees(),
            anisotropic = self.anisotropic,
            inside_hemisphere = self.inside_hemisphere,
            sample_count = self.sample_count,
            min = self.intensity.min(),
            max = self.intensity.max(),
        )
    }
}

//
// Render task
//

struct RenderTask {
    model: bxdfs::Model,
    comp: BrdfComponent,
    inputs: (&'static str, Vec<BrdfInput>),
    seq: SampleSequence,
    flags: u32,
}

pub fn run() -> Result<()> {
    // Default work directory.
    let work_dir = PathBuf::from("work");

    // Default text renderer.
    let text_renderer = text::Renderer::new()?;

    // Sequences.
    let seq_grid = SampleSequence::Grid(DEFAULT_SAMPLE_GRID_WIDTH);
    let seq_rand = SampleSequence::Random(UniformSampler::new());
    let seq_sobol = SampleSequence::Sobol;

    // Brdf inputs.
    let iso_scalar_builder = BrdfInputBuilder::new().lerp_scalar();
    let iso_incoming_builder = BrdfInputBuilder::new().lerp_incoming();
    let ani_scalar_builder = iso_scalar_builder.anisotropic(1.0);
    let ani_incoming_builder = iso_incoming_builder.anisotropic(1.0);
    let iso_scalar = iso_scalar_builder.into_sequence();
    let iso_incoming = iso_incoming_builder.into_sequence();
    let ani_scalar = ani_scalar_builder.into_sequence();
    let ani_incoming = ani_incoming_builder.into_sequence();

    // Create tasks.
    macro_rules! task {
        ($brdf_model:ident, $brdf_comp:ident, $name:literal, $inputs:ident, $seq:ident, $flags:expr) => {
            RenderTask {
                model: bxdfs::Model::$brdf_model,
                comp: BrdfComponent::$brdf_comp,
                inputs: ($name, $inputs.clone()),
                seq: $seq.clone(),
                flags: $flags,
            }
        };
    }
    #[rustfmt::skip]
    let tasks = vec![
        // Uniform vs Cosine.
        task!(Lambertian, R, "uniform", iso_scalar, seq_grid, FLAG_UNIFORM),
        task!(Lambertian, Pdf, "uniform", iso_scalar, seq_grid, FLAG_UNIFORM),
        task!(Lambertian, R, "cosine", iso_scalar, seq_grid, FLAG_COSINE),
        task!(Lambertian, Pdf, "cosine", iso_scalar, seq_grid, FLAG_COSINE),
        // Different sequences.
        task!(Lambertian, R, "grid", iso_scalar, seq_grid, FLAG_COSINE),
        task!(Lambertian, Pdf, "grid", iso_scalar, seq_grid, FLAG_COSINE),
        task!(Lambertian, R, "rand", iso_scalar, seq_rand, FLAG_COSINE),
        task!(Lambertian, Pdf, "rand", iso_scalar, seq_rand, FLAG_COSINE),
        task!(Lambertian, R, "sobol", iso_scalar, seq_sobol, FLAG_COSINE),
        task!(Lambertian, Pdf, "sobol", iso_scalar, seq_sobol, FLAG_COSINE),
        // Lambert.
        task!(Lambertian, R, "scalar", iso_scalar, seq_sobol, 0),
        task!(Lambertian, Pdf, "scalar", iso_scalar, seq_sobol, 0),
        task!(Lambertian, R, "incoming", iso_incoming, seq_sobol, 0),
        task!(Lambertian, Pdf, "incoming", iso_incoming, seq_sobol, 0),
        // Disney Diffuse.
        task!(DisneyDiffuse, R, "scalar", iso_scalar, seq_sobol, 0),
        task!(DisneyDiffuse, Pdf, "scalar", iso_scalar, seq_sobol, 0),
        task!(DisneyDiffuse, R, "incoming", iso_incoming, seq_sobol, 0),
        task!(DisneyDiffuse, Pdf, "incoming", iso_incoming, seq_sobol, 0),
        // Microfacet Reflection.
        task!(MicrofacetReflection, R, "scalar", iso_scalar, seq_sobol, 0),
        task!(MicrofacetReflection, Pdf, "scalar", iso_scalar, seq_sobol, 0),
        task!(MicrofacetReflection, R, "incoming", iso_incoming, seq_sobol, 0),
        task!(MicrofacetReflection, Pdf, "incoming", iso_incoming, seq_sobol, 0),
        // Microfacet Reflection - Anisotropic.
        task!(MicrofacetReflection, R, "ani-scalar", ani_scalar, seq_sobol, 0),
        task!(MicrofacetReflection, Pdf, "ani-scalar", ani_scalar, seq_sobol, 0),
        task!(MicrofacetReflection, R, "ani-incoming", ani_incoming, seq_sobol, 0),
        task!(MicrofacetReflection, Pdf, "ani-incoming", ani_incoming, seq_sobol, 0),
    ];

    // Execute tasks.
    tasks.par_iter().for_each(|task| {
        // Metadata.
        let (input_name, inputs) = task.inputs.clone();
        let header = PlotHeader {
            model: task.model.name(),
            comp: task.comp.name(),
            name: input_name,
            seq: task.seq.name(),
        };

        let mut frames_angle = Vec::with_capacity(inputs.len());
        let mut frames_hemisphere = Vec::with_capacity(inputs.len());
        for input in inputs {
            // Make hemisphere sampler.
            let hemisphere = if task.flags & FLAG_UNIFORM > 0 {
                HemisphereSampler::Uniform
            } else {
                HemisphereSampler::Cosine
            };

            // Make BRDF.
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
                        roughness: input.scalar,
                    }))
                }
                bxdfs::Model::MicrofacetReflection => Box::new(bxdfs::MicrofacetReflection::new(
                    &bxdfs::MicrofacetReflectionParams {
                        base_color: DEFAULT_BASE_COLOR,
                        metallic: 1.0,
                        specular: 0.5,
                        roughness: input.scalar,
                        anisotropic: input.anisotropic,
                    },
                )),
            };

            let mut plot = plot::new(input.incoming, |surface| match task.comp {
                BrdfComponent::R => brdf.eval(surface.outgoing(), surface.incoming()),
                BrdfComponent::Pdf => {
                    let pdf = brdf.pdf(surface.outgoing(), surface.incoming());
                    ColorRgb::new(pdf, pdf, pdf)
                }
            });

            let mut inside_hemisphere = 0;
            let mut sequence = task.seq.clone();
            plot.sample_f((0..DEFAULT_SAMPLE_COUNT).into_iter().map(|sample_index| {
                let uniform = sequence.sample(sample_index);
                match brdf.sample(&input.incoming, uniform) {
                    Some(sample) => {
                        inside_hemisphere += 1;
                        sample.wi()
                    }
                    None => bxdfs::LocalVector(Y_AXIS),
                }
            }));

            let annotation = PlotAnnotation {
                header: &header,
                scalar: input.scalar,
                incoming: input.angle_theta,
                intensity: plot.intensity(),
                anisotropic: input.anisotropic,
                inside_hemisphere,
                sample_count: DEFAULT_SAMPLE_COUNT,
            };
            let text = annotation.to_string();
            let (mut angle_image, mut hemisphere_image) = plot.into_images();
            text_renderer.draw(&mut angle_image, &text);
            text_renderer.draw(&mut hemisphere_image, &text);
            frames_angle.push(angle_image);
            frames_hemisphere.push(hemisphere_image);
        }

        // Boomerang.
        let frames_angle = animation::create_boomerang(frames_angle);
        let frames_hemisphere = animation::create_boomerang(frames_hemisphere);

        // Render animation.
        let path_angle = work_dir.join(header.to_filename(PlotType::Angle));
        let path_hemisphere = work_dir.join(header.to_filename(PlotType::Hemisphere));
        animation::render(path_angle, frames_angle).expect("Failed to render animation");
        animation::render(path_hemisphere, frames_hemisphere).expect("Failed to render animation");
    });

    // Debug website.
    {
        use std::io::Write;

        let html = r#"<!doctype html>
<html lang="en">
<head>
    <style>
        body {
            background-color: rgb(10, 10, 10);
            max-width: 2400px;
            margin: 0 auto;
        }
        h1 {
            color: white;
        }
        .cards {
            width: 2400px;
            height: 620px;
            display: flex;
            flex-direction: row;
            flex-wrap: wrap;
        }
        .card {
            width: 400px;
            height: 620px;
            display: flex;
            flex-direction: column;
        }
    </style>
</head>

<body>
<div class="cards">

{{cards}}
</div>
</body>
</html>
        "#;

        let file = File::create(work_dir.join("debug.html"))?;
        let mut writer = BufWriter::new(file);
        let mut cards = String::new();
        for task in tasks {
            let (input_name, _) = task.inputs;
            let header = PlotHeader {
                model: task.model.name(),
                comp: task.comp.name(),
                name: input_name,
                seq: task.seq.name(),
            };
            let path_angle = header.to_filename(PlotType::Angle);
            let path_hemisphere = header.to_filename(PlotType::Hemisphere);
            cards.push_str(&format!(
                "   <div class=\"card\"><img src=\"{}\"><img src=\"{}\"></div>\n",
                path_hemisphere, path_angle,
            ));
        }
        writer.write_all(
            html.replace("{{cards}}", &cards.replace('\\', "/"))
                .as_bytes(),
        )?;
    }

    Ok(())
}
