use super::*;

//
// Configs
//

#[derive(clap::Args)]
pub struct Args {
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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct MaterialMapping(String, glb::MaterialField, glb::DynamicTexture);

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
    let mut page = vz::page::Builder::new("render");
    for scene_config in scene_configs {
        let name = scene_config.name.clone();
        let image = render(&render_config, scene_config)?;
        page.push_card(name, image);
    }

    // Render page.
    {
        use std::io::Write;
        let file_name = "render.html";
        let path = work_dir().join(file_name);
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);
        write!(&mut writer, "{}", page.build()?)?;
        info!("Wrote to {}", path.display());
        info!("Open with http://127.0.0.1:5500/work/{file_name}");
    }

    Ok(())
}

fn render(render_config: &RenderConfig, scene_config: SceneConfig) -> Result<String> {
    // Default font.
    let font = vz::font::Font::new()?;

    // Unpack render config.
    let samples_per_pixel = render_config.samples_per_pixel.get();
    let image_scale = render_config.image_scale.get();
    let image_aspect = (16, 9);
    let image_size = (image_aspect.0 * image_scale, image_aspect.1 * image_scale);
    let frame_delay_num = render_config.frame_delay_num.get();
    let frame_delay_den = render_config.frame_delay_den.get();
    let tonemapping = render_config.tonemapping;
    let exposure = Exposure::new(render_config.exposure);

    // Unpack scene config.
    let material_mappings = &scene_config.material_mappings;
    let sky_elevation_deg: vz::anim::Value<_> = scene_config.sky_elevation_deg.into();
    let sky_azimuth_deg: vz::anim::Value<_> = scene_config.sky_azimuth_deg.into();
    let sky_turbidity: vz::anim::Value<_> = scene_config.sky_turbidity.into();
    let sky_albedo: vz::anim::Value<_> = scene_config.sky_albedo.into();

    // Animation length.
    let mut total_time = -f32::MAX;
    total_time = total_time.max(sky_elevation_deg.max_time());
    total_time = total_time.max(sky_azimuth_deg.max_time());
    total_time = total_time.max(sky_turbidity.max_time());
    total_time = total_time.max(sky_albedo.max_time());
    let frame_time = f32::from(frame_delay_num) / f32::from(frame_delay_den);
    let frame_count = (total_time / frame_time).ceil() as u32;
    info!("total_time={total_time}, frame_count={frame_count}");

    // Init glb scene.
    let (glb_scene, mut dyn_scene) =
        glb::Scene::create(include_bytes!("../assets/rounded_cube.glb"))?;

    // Apply material mappings.
    for mapping in material_mappings {
        use itertools::Itertools;
        if let Some((material_index, _)) = glb_scene
            .materials
            .iter()
            .find_position(|m| m.name == mapping.0)
        {
            let material = &dyn_scene.materials[material_index];
            let texture_index = match mapping.1 {
                glb::MaterialField::BaseColor => material.base_color,
                glb::MaterialField::Metallic => material.metallic,
                glb::MaterialField::Roughness => material.roughness,
            } as usize;
            dyn_scene.textures[texture_index] = mapping.2;
            dyn_scene.replaced_textures.set(texture_index, true);
        }
    }

    // Init cpupt.
    let raytracer = cpupt::Raytracer::create(
        cpupt::Params {
            samples_per_pixel,
            ..cpupt::Params::default()
        },
        glb_scene,
    );

    // Render frames.
    let frames = {
        use indicatif::{ProgressBar, ProgressStyle};

        let timer = Instant::now();
        let hemisphere_sampler = HemisphereSampler::Cosine;
        let visualize_normals = false;
        let pb = ProgressBar::new(u64::from(frame_count)).with_style(ProgressStyle::with_template(
            "{wide_bar} elapsed={elapsed_precise} eta={eta_precise}",
        )?);
        let mut frames = vec![];
        for frame_index in 0..frame_count {
            let time = frame_index as f32 * frame_time;
            let camera_angle = 0.0;
            let camera_transform = Mat4::from_axis_angle(&Vec3::y_axis(), camera_angle);
            let sky_elevation = sky_elevation_deg.value(time).to_radians();
            let sky_azimuth = sky_azimuth_deg.value(time).to_radians();
            let sky_turbidity = sky_turbidity.value(time);
            let sky_albedo = sky_albedo.value(time);
            raytracer.send_input(cpupt::Input {
                camera_transform,
                image_size,
                hemisphere_sampler,
                dyn_scene: dyn_scene.clone(),
                visualize_normals,
                tonemapping,
                exposure,
                sky_params: cpupt::sky::ext::StateExtParams {
                    elevation: sky_elevation,
                    azimuth: sky_azimuth,
                    turbidity: sky_turbidity,
                    albedo: sky_albedo,
                },
            });
            let mut latest_frame: Option<vz::image::Rgb> = None;
            for _ in 0..samples_per_pixel {
                let output = raytracer.recv_output().expect("Something went wrong");
                latest_frame = Some(vz::image::Rgb::from_colors(
                    &output.image,
                    output.image_size,
                ));
            }
            let mut latest_frame = latest_frame.unwrap();
            if render_config.annotations {
                latest_frame.draw_text(
                    &font,
                    ColorRgb::WHITE,
                    &vz::annotation::TextBox::new()
                        .line([("scene", &scene_config.name)])
                        .line([
                            ("elevation", format!("{sky_elevation:.02} deg")),
                            ("azimuth", format!("{sky_azimuth:.02} deg")),
                        ])
                        .line([
                            ("turbidity", format!("{sky_turbidity:.02}")),
                            ("albedo", format!("{sky_albedo:.02}")),
                            ("exposure", format!("{exposure:.02}")),
                        ])
                        .line([
                            ("time", format!("{time:.02}/{total_time:.02}")),
                            ("frame", format!("{frame_index}/{frame_count}")),
                            ("res", format!("{}x{}", image_size.0, image_size.1)),
                            ("spp", format!("{samples_per_pixel}")),
                            ("tonemap", format!("{tonemapping}")),
                        ])
                        .build(),
                );
            }
            frames.push(latest_frame);
            pb.inc(1);
        }
        pb.finish();
        info!("Rendering took {} seconds", timer.elapsed().as_secs_f64());
        frames
    };

    // Make boomerang.
    let frames = vz::video::create_boomerang(frames);

    // Render animation.
    let file_name = format!("render-{}.apng", scene_config.name);
    vz::video::render(
        &vz::video::Params {
            delay_num: frame_delay_num,
            delay_den: frame_delay_den,
        },
        work_dir().join(&file_name),
        frames,
    )?;

    // Cleanup.
    raytracer.terminate();

    Ok(file_name)
}
