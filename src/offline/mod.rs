use super::*;

//
// Configs
//

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    glb_scene: PathBuf,

    #[arg(long)]
    render_job_name: String,

    #[arg(long)]
    render_config: PathBuf,

    #[arg(long)]
    scene_config: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct RenderConfig {
    samples_per_pixel: NonZeroU32,
    image_scale: NonZeroU32,
    frame_delay_num: NonZeroU16,
    frame_delay_den: NonZeroU16,
    tonemapping: bool,
    exposure: f32,
    annotations: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct SceneConfig {
    name: String,
    material_mappings: Vec<MaterialMapping>,
    sky_elevation_deg: vz::cfg::Value<f32>,
    sky_azimuth_deg: vz::cfg::Value<f32>,
    sky_turbidity: vz::cfg::Value<f32>,
    sky_albedo: vz::cfg::Value<ColorRgb>,
    text_annotations: Vec<TextAnnotation>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct MaterialMapping(
    String,
    rds::MaterialField,
    vz::cfg::Value<rds::DynamicTexture>,
);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
enum TextAnnotation {
    SkyParameters,
    Material(String),
}

//
// Runners
//

pub fn run(args: Args) -> Result<()> {
    // Load configs.
    let render_config: RenderConfig = vz::cfg::read_from_file(args.render_config)?;
    let scene_configs: Vec<SceneConfig> = vz::cfg::read_from_file(args.scene_config)?;

    // Backup configs.
    vz::cfg::write_to_file(work_dir().join("render.ron"), &render_config)?;
    vz::cfg::write_to_file(work_dir().join("scene.ron"), &scene_configs)?;

    // Render configs.
    let mut page = vz::page::Builder::new(&args.render_job_name);
    for scene_config in scene_configs {
        let name = scene_config.name.clone();
        let image = render(
            &args.glb_scene,
            &args.render_job_name,
            &render_config,
            scene_config,
        )?;
        page.push_card("render", name, image);
    }

    // Render page.
    {
        use std::io::Write;
        let file_name = format!("render-{}.html", args.render_job_name);
        let path = work_dir().join(&file_name);
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);
        write!(&mut writer, "{}", page.build()?)?;
        info!("Wrote to {}", path.display());
        info!("Open with http://127.0.0.1:5500/work/{file_name}");
    }

    Ok(())
}

fn render(
    glb_scene: &Path,
    render_job_name: &str,
    render_config: &RenderConfig,
    scene_config: SceneConfig,
) -> Result<String> {
    // Default font.
    let font = vz::font::Font::new()?;

    // Unpack render config.
    let samples_per_pixel = render_config.samples_per_pixel.get();
    let image_scale = render_config.image_scale.get();
    let image_aspect = DEFAULT_ASPECT_RATIO;
    let image_size = (image_aspect.0 * image_scale, image_aspect.1 * image_scale);
    info!("Rendering image {}x{}", image_size.0, image_size.1);
    let frame_delay_num = render_config.frame_delay_num.get();
    let frame_delay_den = render_config.frame_delay_den.get();
    let tonemapping = render_config.tonemapping;
    let exposure = cpupt::Exposure::new(render_config.exposure);

    // Unpack scene config.
    let material_mappings = scene_config.material_mappings;
    let sky_elevation_deg: vz::anim::Value<_> = scene_config.sky_elevation_deg.into();
    let sky_azimuth_deg: vz::anim::Value<_> = scene_config.sky_azimuth_deg.into();
    let sky_turbidity: vz::anim::Value<_> = scene_config.sky_turbidity.into();
    let sky_albedo: vz::anim::Value<_> = scene_config.sky_albedo.into();
    let text_annotations = scene_config.text_annotations;

    // Init rds scene.
    let (rds_scene, mut dyn_scene) = rds::Scene::create(
        &std::fs::read(glb_scene)
            .with_context(|| format!("Reading glb scene: {}", glb_scene.display()))?,
    )?;

    // Init materials.
    let material_mappings = material_mappings
        .into_iter()
        .filter_map(|map| {
            let MaterialMapping(name, field, value) = map;
            if let Some(material) = rds::dynamic_material_by_name(&rds_scene, &dyn_scene, &name) {
                let texture = material.texture(field);
                dyn_scene.replaced_textures.set(texture as usize, true);
                Some((texture, vz::anim::Value::<_>::from(value)))
            } else {
                warn!("Could not find material called {name}");
                None
            }
        })
        .collect::<Vec<_>>();

    // Calculate animation length.
    let mut total_time = -f32::MAX;
    total_time = total_time.max(sky_elevation_deg.max_time());
    total_time = total_time.max(sky_azimuth_deg.max_time());
    total_time = total_time.max(sky_turbidity.max_time());
    total_time = total_time.max(sky_albedo.max_time());
    for (_, value) in &material_mappings {
        total_time = total_time.max(value.max_time());
    }
    let frame_time = f32::from(frame_delay_num) / f32::from(frame_delay_den);
    let frame_count = (total_time / frame_time).ceil() as u64;
    info!("total_time={total_time}, frame_count={frame_count}");

    // // Init cpupt.
    // let raytracer = cpupt::Raytracer::create(
    //     cpupt::Params {
    //         samples_per_pixel,
    //         ..cpupt::Params::default()
    //     },
    //     rds_scene.clone(),
    // );
    let mut renderer = vkpt::Renderer::create(vkpt::RendererCreateInfo {
        image_size,
        rds_scene: rds_scene.clone(),
    })?;

    // Render frames.
    let frames = {
        use indicatif::{ProgressBar, ProgressStyle};

        let timer = Instant::now();
        let hemisphere_sampler = cpupt::HemisphereSampler::Cosine;
        let visualize_normals = false;
        // let pb = ProgressBar::new(u64::from(frame_count * samples_per_pixel)).with_style(
        //     ProgressStyle::with_template("{wide_bar} elapsed={elapsed_precise} eta={eta_precise}")?,
        // );
        // let pb = ProgressBar::new(frame_count).with_style(ProgressStyle::with_template(
        //     "{wide_bar} elapsed={elapsed_precise} eta={eta_precise}",
        // )?);
        let mut frames = vec![];
        for frame_index in 0..frame_count {
            // Time.
            let time = frame_index as f32 * frame_time;

            // Camera.
            let camera_angle = 0.0;
            let camera_transform = Mat4::from_axis_angle(&Vec3::y_axis(), camera_angle);

            // Sky.
            let sky_elevation = sky_elevation_deg.value(time).to_radians();
            let sky_azimuth = sky_azimuth_deg.value(time).to_radians();
            let sky_turbidity = sky_turbidity.value(time);
            let sky_albedo = sky_albedo.value(time);

            // Materials.
            for (index, value) in &material_mappings {
                dyn_scene.textures[*index as usize] = value.value(time);
            }

            // // Render.
            // raytracer.send_input(cpupt::Input {
            //     camera_transform,
            //     image_size,
            //     hemisphere_sampler,
            //     dyn_scene: dyn_scene.clone(),
            //     visualize_normals,
            //     tonemapping,
            //     exposure,
            //     sky_params: cpupt::SkyParams {
            //         elevation: sky_elevation,
            //         azimuth: sky_azimuth,
            //         turbidity: sky_turbidity,
            //         albedo: sky_albedo,
            //     },
            //     salt: Some(frame_index.into()),
            // })?;
            // let mut latest_frame: Option<vz::image::Rgb> = None;
            // for _ in 0..samples_per_pixel {
            //     let output = raytracer.recv_output().expect("Something went wrong");
            //     latest_frame = Some(vz::image::Rgb::from_colors(
            //         &output.image,
            //         output.image_size,
            //     ));
            //     pb.inc(1);
            // }
            // let mut latest_frame = latest_frame.unwrap();
            let input = vkpt::RendererInput {
                frame_index,
                frame_count,
                camera_transform,
                image_size,
                dyn_scene: dyn_scene.clone(),
            };
            renderer.update(&input);
            let mut latest_frame = renderer.render(&input)?;
            // pb.inc(1);

            if render_config.annotations {
                let mut text = vz::annotation::TextBox::new();

                // Scene name.
                text = text.line([("scene", &scene_config.name)]);

                // Annotate materials.
                for ta in &text_annotations {
                    let TextAnnotation::Material(name) = ta else {
                        continue;
                    };

                    let Some(material) = rds::dynamic_material_by_name(&rds_scene, &dyn_scene, name) else {
                        warn!("Could not find material called {name}");
                        continue;
                    };

                    let base_color = dyn_scene.textures[material.base_color as usize];
                    let metallic = dyn_scene.textures[material.metallic as usize];
                    let roughness = dyn_scene.textures[material.roughness as usize];
                    let specular = dyn_scene.textures[material.specular as usize];
                    let specular_tint = dyn_scene.textures[material.specular_tint as usize];
                    let sheen = dyn_scene.textures[material.sheen as usize];
                    let sheen_tint = dyn_scene.textures[material.sheen_tint as usize];
                    text = text.line([("material", name.as_str())]);
                    text = text.line([("    base_color", &format!("{base_color:.02}"))]);
                    text = text.line([
                        ("    metallic", &format!("{metallic:.02}")),
                        ("roughness", &format!("{roughness:.02}")),
                    ]);
                    text = text.line([
                        ("    specular", &format!("{specular:.02}")),
                        ("specular_tint", &format!("{specular_tint:.02}")),
                    ]);
                    text = text.line([
                        ("    sheen", &format!("{sheen:.02}")),
                        ("sheen_tint", &format!("{sheen_tint:.02}")),
                    ]);
                }

                // Annotate sky parameters.
                if text_annotations
                    .iter()
                    .any(|ta| matches!(&ta, TextAnnotation::SkyParameters))
                {
                    text = text
                        .line([
                            ("elevation", format!("{:.02}°", sky_elevation.to_degrees())),
                            ("azimuth", format!("{:.02}°", sky_azimuth.to_degrees())),
                        ])
                        .line([
                            ("turbidity", format!("{sky_turbidity:.02}")),
                            ("albedo", format!("{sky_albedo:.02}")),
                        ]);
                }

                // Render configs.
                text = text.line([
                    ("time", format!("{time:.02}/{total_time:.02}")),
                    ("frame", format!("{frame_index}/{frame_count}")),
                    ("res", format!("{}x{}", image_size.0, image_size.1)),
                    ("spp", format!("{samples_per_pixel}")),
                ]);
                text = text.line([
                    ("tonemap", format!("{tonemapping}")),
                    ("exposure", format!("{exposure:.02}")),
                ]);

                latest_frame.draw_text(&font, ColorRgb::WHITE, &text.build());
            }
            frames.push(latest_frame);
        }
        // pb.finish();
        info!("Rendering took {} seconds", timer.elapsed().as_secs_f64());
        frames
    };

    // Make boomerang.
    let frames = vz::apng::create_boomerang(frames);

    // Render animation.
    let file_name = format!("render-{render_job_name}-{}.apng", scene_config.name);
    vz::apng::render(
        &vz::apng::Params {
            delay_num: frame_delay_num,
            delay_den: frame_delay_den,
        },
        work_dir().join(&file_name),
        frames,
    )?;

    // // Cleanup.
    // raytracer.terminate()?;
    renderer.destroy();

    Ok(file_name)
}

#[test]
fn config_template() {
    use rds::DynamicTexture::Scalar as TS;
    use rds::DynamicTexture::Vector4 as TV4;
    use rds::MaterialField::{BaseColor, Metallic, Roughness};
    use vz::cfg::{keyframe, Value};
    use Value::{Constant, Keyframes};

    let config = SceneConfig {
        name: "roughness".to_owned(),
        material_mappings: vec![
            MaterialMapping(
                "cube".to_owned(),
                BaseColor,
                Keyframes(vec![
                    keyframe!(0.0, TV4([1.0, 1.0, 1.0, 1.0]), CubicInOut),
                    keyframe!(1.0, TV4([1.0, 0.5, 0.5, 1.0]), CubicInOut),
                    keyframe!(2.0, TV4([0.5, 1.0, 0.5, 1.0]), CubicInOut),
                    keyframe!(3.0, TV4([0.5, 0.5, 1.0, 1.0]), CubicInOut),
                ]),
            ),
            MaterialMapping(
                "cube".to_owned(),
                Roughness,
                Keyframes(vec![
                    keyframe!(0.0, TS(0.0), CubicInOut),
                    keyframe!(1.5, TS(1.0), CubicInOut),
                    keyframe!(3.0, TS(0.0), CubicInOut),
                ]),
            ),
            MaterialMapping("cube".to_owned(), Metallic, Constant(TS(0.75))),
            MaterialMapping(
                "plane".to_owned(),
                Roughness,
                Keyframes(vec![
                    keyframe!(0.0, TS(1.0), CubicInOut),
                    keyframe!(1.5, TS(0.0), CubicInOut),
                    keyframe!(3.0, TS(0.5), CubicInOut),
                ]),
            ),
            MaterialMapping("plane".to_owned(), Metallic, Constant(TS(0.75))),
        ],
        sky_elevation_deg: Constant(45.0),
        sky_azimuth_deg: Keyframes(vec![
            keyframe!(0.0, 0.0, CubicInOut),
            keyframe!(3.0, 180.0, CubicInOut),
        ]),
        sky_turbidity: Constant(3.0),
        sky_albedo: Constant(ColorRgb::WHITE),
        text_annotations: vec![
            TextAnnotation::SkyParameters,
            TextAnnotation::Material("cube".to_owned()),
        ],
    };

    println!(
        "{}",
        ron::ser::to_string_pretty(&vec![config], ron::ser::PrettyConfig::default()).unwrap()
    );
}
