use super::*;

pub mod bxdfs;
pub mod sampling;
pub mod sky;

mod bvh;
mod intersection;
mod ray;
mod triangle;

use ray::*;
use triangle::*;

pub struct Scene {
    bvh_nodes: Vec<bvh::Node>,
    triangles: Vec<Triangle>,
}

impl Scene {
    pub fn create(rds_scene: &rds::Scene) -> Self {
        let max_triangle_count = rds_scene
            .meshes
            .iter()
            .map(rds::Mesh::triangle_count)
            .sum::<u32>();
        let mut triangles = Vec::with_capacity(max_triangle_count as usize);
        for mesh in &rds_scene.meshes {
            for triangle in &mesh.triangles {
                let position_0 = mesh.positions[triangle[0] as usize];
                let position_1 = mesh.positions[triangle[1] as usize];
                let position_2 = mesh.positions[triangle[2] as usize];
                let tex_coord_0 = mesh.tex_coords[triangle[0] as usize];
                let tex_coord_1 = mesh.tex_coords[triangle[1] as usize];
                let tex_coord_2 = mesh.tex_coords[triangle[2] as usize];
                let normal_0 = mesh.normals[triangle[0] as usize];
                let normal_1 = mesh.normals[triangle[1] as usize];
                let normal_2 = mesh.normals[triangle[2] as usize];

                let position_0 = mesh.transform.transform_point(&position_0);
                let position_1 = mesh.transform.transform_point(&position_1);
                let position_2 = mesh.transform.transform_point(&position_2);
                let normal_0 = normal!(mesh.transform.transform_vector(&normal_0));
                let normal_1 = normal!(mesh.transform.transform_vector(&normal_1));
                let normal_2 = normal!(mesh.transform.transform_vector(&normal_2));

                triangles.push(Triangle {
                    positions: [position_0, position_1, position_2],
                    normals: [normal_0, normal_1, normal_2],
                    tex_coords: [tex_coord_0, tex_coord_1, tex_coord_2],
                    material: mesh.material,
                });
            }
        }
        let (bvh_nodes, triangles) = bvh::create(&triangles);
        Self {
            bvh_nodes,
            triangles,
        }
    }
}

pub struct Params {
    pub samples_per_pixel: u32,
    pub max_bounce_count: u32,
    pub seed: u64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            samples_per_pixel: 64,
            max_bounce_count: 5,
            seed: 0,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Input {
    pub camera_transform: Mat4,
    pub image_size: (u32, u32),
    pub hemisphere_sampler: sampling::HemisphereSampler,
    pub dyn_scene: rds::DynamicScene,
    pub visualize_normals: bool,
    pub tonemapping: bool,
    pub exposure: Exposure,
    pub sky_params: sky::ext::StateExtParams,
    pub salt: Option<u64>,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            camera_transform: Mat4::identity(),
            image_size: (0, 0),
            hemisphere_sampler: sampling::HemisphereSampler::default(),
            dyn_scene: rds::DynamicScene::default(),
            visualize_normals: false,
            tonemapping: true,
            exposure: Exposure::default(),
            sky_params: sky::ext::StateExtParams::default(),
            salt: None,
        }
    }
}

impl Input {
    fn is_valid(&self) -> bool {
        if self.image_size.0 == 0 {
            return false;
        }
        if self.image_size.1 == 0 {
            return false;
        }
        true
    }
}

pub struct Output {
    pub image: Vec<ColorRgb>,
    pub image_size: (u32, u32),
    pub sample_index: u32,
    pub sample_count: u32,
}

pub struct Raytracer {
    thread: thread::JoinHandle<Result<()>>,
    input_send: mpsc::Sender<Input>,
    output_recv: mpsc::Receiver<Output>,
    terminate_send: mpsc::Sender<()>,
}

impl Raytracer {
    pub fn create(params: Params, rds_scene: rds::Scene) -> Self {
        let (input_send, input_recv) = mpsc::channel();
        let (output_send, output_recv) = mpsc::channel();
        let (terminate_send, terminate_recv) = mpsc::channel();
        let thread = thread::spawn(move || {
            let params = params;
            let rds_scene = rds_scene;
            let scene = Scene::create(&rds_scene);
            let materials = rds_scene.materials.as_ref();
            let input_recv: mpsc::Receiver<Input> = input_recv;
            let output_send = output_send;
            let terminate_recv = terminate_recv;

            let mut input = Input::default();
            let mut sample_index = 0;
            let mut world_from_clip = Mat4::identity();
            let mut camera_position = Point3::origin();
            let mut timer = Instant::now();
            let mut ray_stats = intersection::RayBvhHitStats::default();
            let mut tiles = vec![];
            let mut tile_results = vec![];
            let mut pixel_buffer = Vec::<ColorRgb>::new();
            let mut sky_state =
                sky::ext::StateExt::new(&sky::ext::StateExtParams::default()).unwrap();

            loop {
                // Check for termination command.
                if terminate_recv.try_recv().is_ok() {
                    info!("Terminating raytracer");
                    break;
                }

                // Get latest input.
                let latest_input = {
                    let mut latest_input = None;
                    while let Ok(input) = input_recv.try_recv() {
                        latest_input = Some(input);
                    }
                    latest_input
                };

                // If the inputs have changed, reset state.
                if let Some(latest_input) = latest_input {
                    if latest_input != input {
                        debug!("Reset raytracer with new input");

                        // Update input.
                        input = latest_input;

                        // Reset sampling state.
                        sample_index = 0;
                        pixel_buffer.clear();
                        pixel_buffer.resize(
                            (input.image_size.0 * input.image_size.1) as usize,
                            ColorRgb::BLACK,
                        );

                        // Reset camera.
                        let camera = &rds_scene.cameras[0];
                        let camera_transform = input.camera_transform.try_inverse().unwrap();
                        let view_from_clip = camera.clip_from_view().inverse();
                        let world_from_view = camera.world_from_view();
                        world_from_clip = camera_transform * world_from_view * view_from_clip;
                        camera_position = camera_transform.transform_point(&camera.position());

                        // Reset sky.
                        sky_state = sky::ext::StateExt::new(&input.sky_params).unwrap();

                        // Reset stats.
                        ray_stats = intersection::RayBvhHitStats::default();

                        // Reset tiles.
                        tiles = PixelTiles::new(input.image_size.0, input.image_size.1)
                            .collect::<Vec<_>>();
                        tile_results.reserve(tiles.len());

                        // Reset timer.
                        timer = Instant::now();
                    }
                }

                // If the state is invalid, skip.
                if !input.is_valid() {
                    continue;
                }

                // Rendering.
                if sample_index < params.samples_per_pixel {
                    // Unpack.
                    let image_size = input.image_size;

                    // Render tiles.
                    (0..tiles.len())
                        .into_par_iter()
                        .map(|tile_index| {
                            let tile = tiles[tile_index];
                            let (tile_radiance, tile_ray_stats) = tile_radiance(
                                &tile,
                                image_size,
                                sample_index,
                                camera_position,
                                world_from_clip,
                                &input,
                                &params,
                                &scene,
                                &rds_scene,
                                &input.dyn_scene,
                                materials,
                                &sky_state,
                            );
                            (tile, tile_radiance, tile_ray_stats)
                        })
                        .collect_into_vec(&mut tile_results);

                    // Accumulate results.
                    for (tile, tile_radiance, tile_ray_stats) in &tile_results {
                        let mut src_pixel_index = 0;
                        for pixel_y in tile.start_y..tile.end_y {
                            for pixel_x in tile.start_x..tile.end_x {
                                let dst_pixel_index = (pixel_x + pixel_y * image_size.0) as usize;
                                let src_pixel = tile_radiance[src_pixel_index];
                                let dst_pixel = &mut pixel_buffer[dst_pixel_index];
                                *dst_pixel += src_pixel;
                                src_pixel_index += 1;
                            }
                        }
                        ray_stats += *tile_ray_stats;
                    }

                    // Normalize the current image, send it.
                    let normalization_factor = 1.0 / (sample_index + 1) as f32;
                    let mut max_radiance = 0.0_f32;
                    let image = pixel_buffer
                        .clone()
                        .into_iter()
                        .map(|sample| {
                            // Averaging samples.
                            sample * normalization_factor
                        })
                        .map(|sample| {
                            // Statistics.
                            max_radiance = max_radiance.max(sample.r());
                            max_radiance = max_radiance.max(sample.g());
                            max_radiance = max_radiance.max(sample.b());
                            sample
                        })
                        .map(|sample| {
                            // Exposure and tonemapping.
                            let sample = input.exposure.expose(sample);
                            if input.tonemapping {
                                sample.tonemap()
                            } else {
                                sample
                            }
                        })
                        .collect();
                    sample_index += 1;
                    output_send.send(Output {
                        image,
                        image_size,
                        sample_index,
                        sample_count: params.samples_per_pixel,
                    })?;

                    // Rendering has completed.
                    if sample_index == params.samples_per_pixel {
                        let elapsed = timer.elapsed().as_secs_f64();
                        debug!(
                            "Rendering took {:.03} s, {:.03} rays/s, \
                            {:.03} samples/s, {:.03} max radiance",
                            elapsed,
                            ray_stats.rays as f64 / elapsed,
                            f64::from(params.samples_per_pixel) / elapsed,
                            max_radiance
                        );
                        debug!("Stats:\n{ray_stats}");
                    }
                } else {
                    // Avoid busy looping.
                    thread::sleep(Duration::from_millis(1));
                }
            }

            Ok(())
        });
        Self {
            thread,
            input_send,
            output_recv,
            terminate_send,
        }
    }

    pub fn send_input(&self, input: Input) {
        self.input_send.send(input).unwrap();
    }

    pub fn recv_output(&self) -> Option<Output> {
        match self.output_recv.recv() {
            Ok(output) => Some(output),
            Err(err) => panic!("Failed to receive output: {err}"),
        }
    }

    pub fn try_recv_output(&self) -> Option<Output> {
        match self.output_recv.try_recv() {
            Ok(output) => Some(output),
            Err(err) => match err {
                mpsc::TryRecvError::Empty => None,
                mpsc::TryRecvError::Disconnected => panic!("Failed to receive output"),
            },
        }
    }

    pub fn terminate(self) {
        self.terminate_send.send(()).unwrap();
        self.thread.join().unwrap().unwrap();
    }
}

const PIXEL_TILE_SIZE: usize = 16;

const fn pixel_tile_count() -> usize {
    PIXEL_TILE_SIZE * PIXEL_TILE_SIZE
}

fn tile_radiance(
    tile: &PixelTile,
    image_size: (u32, u32),
    sample_index: u32,
    camera_position: Point3,
    world_from_clip: Mat4,
    input: &Input,
    params: &Params,
    scene: &Scene,
    rds_scene: &rds::Scene,
    dyn_scene: &rds::DynamicScene,
    materials: &[rds::Material],
    sky_state: &sky::ext::StateExt,
) -> ([ColorRgb; pixel_tile_count()], intersection::RayBvhHitStats) {
    let mut tile_radiance: [ColorRgb; pixel_tile_count()] = [ColorRgb::BLACK; pixel_tile_count()];
    let mut tile_pixel_index = 0;
    let mut tile_ray_stats = intersection::RayBvhHitStats::default();
    for pixel_y in tile.start_y..tile.end_y {
        for pixel_x in tile.start_x..tile.end_x {
            let (radiance, ray_stats) = radiance(
                (pixel_x, pixel_y),
                image_size,
                sample_index,
                camera_position,
                world_from_clip,
                input,
                params,
                scene,
                rds_scene,
                dyn_scene,
                materials,
                sky_state,
            );
            tile_radiance[tile_pixel_index] = radiance;
            tile_pixel_index += 1;
            tile_ray_stats += ray_stats;
        }
    }

    (tile_radiance, tile_ray_stats)
}

fn radiance(
    pixel: (u32, u32),
    image_size: (u32, u32),
    sample_index: u32,
    camera_position: Point3,
    world_from_clip: Mat4,
    input: &Input,
    params: &Params,
    scene: &Scene,
    rds_scene: &rds::Scene,
    dyn_scene: &rds::DynamicScene,
    materials: &[rds::Material],
    sky_state: &sky::ext::StateExt,
) -> (ColorRgb, intersection::RayBvhHitStats) {
    use bxdfs::Bxdf;

    // Unpack.
    let hemisphere = input.hemisphere_sampler;

    // Init stats.
    let mut ray_stats = intersection::RayBvhHitStats::default();

    // Computes unique seed for every pixel over all samples.
    let seed = u64::from(sample_index + 1) * u64::from(pixel.0 + pixel.1 * image_size.0);
    let mut uniform = sampling::UniformSampler::new_with_seed(seed);

    // Create primary ray.
    let mut ray = {
        sampling::primary_ray(
            pixel,
            image_size,
            &camera_position,
            &world_from_clip,
            uniform.sample(),
            uniform.sample(),
        )
    };

    // Main tracing loop.
    let mut radiance = ColorRgb::BLACK;
    let mut throughput = ColorRgb::WHITE;
    for _ in 0..params.max_bounce_count {
        // Hit scene.
        let mut closest_hit = 0.0;
        let mut barycentrics = Vec3::zeros();
        let mut triangle_index = 0;
        let found_hit = intersection::ray_bvh_hit(
            &ray,
            &scene.bvh_nodes,
            &scene.triangles,
            &mut closest_hit,
            &mut barycentrics,
            &mut triangle_index,
            &mut ray_stats,
        );

        // Special case: ray hit the sky.
        if !found_hit {
            radiance += throughput * sky_state.radiance(&ray.dir);
            break;
        }

        // Unpack triangle data.
        let triangle = &scene.triangles[triangle_index as usize];
        let tex_coord = triangle.interpolated_tex_coord(&barycentrics);
        let normal = triangle.interpolated_normal(&barycentrics);

        // Sample textures.
        let material = &materials[triangle.material as usize];
        let model = rds::dynamic_model(dyn_scene, triangle.material);
        let base_color =
            rds::dynamic_sample(rds_scene, dyn_scene, material.base_color, tex_coord).rgb();
        let roughness =
            rds::dynamic_sample(rds_scene, dyn_scene, material.roughness, tex_coord).r();
        let metallic = rds::dynamic_sample(rds_scene, dyn_scene, material.metallic, tex_coord).r();
        let specular = rds::dynamic_sample(rds_scene, dyn_scene, material.specular, tex_coord).r();
        let specular_tint =
            rds::dynamic_sample(rds_scene, dyn_scene, material.specular_tint, tex_coord).r();
        let anisotropic = 0.0;

        // Orthonormal basis.
        let onb = sampling::OrthonormalBasis::new(&normal);

        // Outgoing vector `wo`. It points to where the ray came from.
        let wo_world = -ray.dir;
        let wo_local = bxdfs::LocalVector::local_from_world(onb.local_from_world(), &wo_world);

        // Evaluate material.
        let bxdf_sample = match model {
            rds::MaterialModel::Diffuse => {
                let bxdf = bxdfs::Lambertian::new(&bxdfs::LambertianParams {
                    hemisphere,
                    base_color,
                });
                match bxdf.sample(&wo_local, (uniform.sample(), uniform.sample())) {
                    Some(s) => s,
                    None => break,
                }
            }
            rds::MaterialModel::Disney => {
                // Todo: pre-calculate these elsewhere.
                let diffuse_brdf = bxdfs::DisneyDiffuse::new(&bxdfs::DisneyDiffuseParams {
                    hemisphere,
                    base_color,
                    roughness,
                });
                let specular_brdf = bxdfs::CookTorrance::new(&bxdfs::CookTorranceParams {
                    base_color,
                    metallic,
                    specular,
                    specular_tint,
                    roughness,
                    anisotropic,
                });
                let diffuse_weight = (1.0 - metallic) * (1.0 - specular);
                let maybe_sample = if uniform.sample() < diffuse_weight {
                    diffuse_brdf.sample(&wo_local, (uniform.sample(), uniform.sample()))
                } else {
                    specular_brdf.sample(&wo_local, (uniform.sample(), uniform.sample()))
                };
                match maybe_sample {
                    Some(s) => s,
                    None => break,
                }
            }
        };
        let wi_world = bxdf_sample.wi().world_from_local(onb.world_from_local());

        // Prepare next direction, adjust closest hit to avoid spawning the next
        // ray inside the surface.
        ray.origin += 0.999 * closest_hit * ray.dir.into_inner();
        ray.dir = wi_world;

        // Update throughput.
        let cos_theta = wi_world.dot(&normal).abs();
        if input.visualize_normals {
            throughput *= ColorRgb::new(
                0.5 * (normal.x + 1.0) * cos_theta,
                0.5 * (normal.y + 1.0) * cos_theta,
                0.5 * (normal.z + 1.0) * cos_theta,
            );
        } else {
            throughput *= bxdf_sample.r() * cos_theta / bxdf_sample.pdf();
        }

        // Report invalid values.
        assert!(
            throughput.is_finite(),
            "material={}, radiance={}, throughput={}, \
            cos_theta={}, origin={:?}, dir={:?}, bxdf_sample={}",
            material.name,
            radiance,
            throughput,
            cos_theta,
            ray.origin,
            ray.dir,
            bxdf_sample
        );
    }
    assert!(radiance.is_finite(), "radiance={radiance}");

    (radiance, ray_stats)
}

#[derive(Clone, Copy)]
struct PixelTile {
    start_x: u32,
    end_x: u32,
    start_y: u32,
    end_y: u32,
}

struct PixelTiles {
    image_w: u32,
    image_h: u32,
    tile_w: u32,
    tile_index: u32,
    tile_count: u32,
}

impl PixelTiles {
    pub fn new(image_w: u32, image_h: u32) -> Self {
        assert!(image_w > 0, "image_w={image_w}");
        assert!(image_h > 0, "image_h={image_h}");
        let tile = PIXEL_TILE_SIZE as u32;
        let tile_w = (image_w + tile - 1) / tile;
        let tile_h = (image_h + tile - 1) / tile;
        let tile_count = tile_w * tile_h;
        Self {
            image_w,
            image_h,
            tile_w,
            tile_index: 0,
            tile_count,
        }
    }
}

impl Iterator for PixelTiles {
    type Item = PixelTile;

    fn next(&mut self) -> Option<Self::Item> {
        if self.tile_index == self.tile_count {
            return None;
        }
        let tile = PIXEL_TILE_SIZE as u32;
        let tile_x = self.tile_index % self.tile_w;
        let tile_y = self.tile_index / self.tile_w;
        let start_x = tile_x * tile;
        let start_y = tile_y * tile;
        let end_x = (start_x + tile).min(self.image_w);
        let end_y = (start_y + tile).min(self.image_h);
        self.tile_index += 1;
        Some(PixelTile {
            start_x,
            end_x,
            start_y,
            end_y,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_iterator() {
        let image_w = 800_u32;
        let image_h = 450_u32;
        let mut hit_pixels = bitvec::bitvec!();
        hit_pixels.resize((image_w * image_h) as usize, false);
        for tile in PixelTiles::new(image_w, image_h) {
            for y in tile.start_y..tile.end_y {
                for x in tile.start_x..tile.end_x {
                    hit_pixels.set((x + y * image_w) as usize, true);
                }
            }
        }
        assert!(hit_pixels.all());
    }
}
