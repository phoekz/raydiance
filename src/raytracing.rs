use super::*;

pub struct Scene {
    bvh_nodes: Vec<bvh::Node>,
    triangles: Vec<Triangle>,
}

impl Scene {
    pub fn create(glb_scene: &glb::Scene) -> Self {
        let max_triangle_count = glb_scene
            .meshes
            .iter()
            .map(glb::Mesh::triangle_count)
            .sum::<u32>();
        let mut triangles = Vec::with_capacity(max_triangle_count as usize);
        for mesh in &glb_scene.meshes {
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
                let normal_0 = na::Unit::new_normalize(mesh.transform.transform_vector(&normal_0));
                let normal_1 = na::Unit::new_normalize(mesh.transform.transform_vector(&normal_1));
                let normal_2 = na::Unit::new_normalize(mesh.transform.transform_vector(&normal_2));

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
            max_bounce_count: 4,
            seed: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Input {
    pub camera_transform: na::Matrix4<f32>,
    pub image_size: (u32, u32),
    pub hemisphere_sampler: sampling::HemisphereSampler,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            camera_transform: na::Matrix4::identity(),
            image_size: (0, 0),
            hemisphere_sampler: sampling::HemisphereSampler::Uniform,
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
    pub image: Vec<LinSrgb>,
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
    pub fn create(params: Params, glb_scene: glb::Scene) -> Self {
        let (input_send, input_recv) = mpsc::channel();
        let (output_send, output_recv) = mpsc::channel();
        let (terminate_send, terminate_recv) = mpsc::channel();
        let thread = thread::spawn(move || {
            let params = params;
            let glb_scene = glb_scene;
            let scene = raytracing::Scene::create(&glb_scene);
            let materials = glb_scene.materials.as_ref();
            let textures = glb_scene.textures.as_ref();
            let input_recv: mpsc::Receiver<Input> = input_recv;
            let output_send = output_send;
            let terminate_recv = terminate_recv;

            let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(params.seed);
            let uniform_01 = rand::distributions::Uniform::new_inclusive(0.0_f32, 1.0_f32);

            let mut input = Input::default();
            let mut sample_index = 0;
            let mut world_from_clip = na::Matrix4::<f32>::identity();
            let mut camera_position = na::Point3::<f32>::origin();
            let mut timer = Instant::now();
            let mut ray_stats = intersection::RayBvhHitStats::default();
            let mut pixel_sample_buffer = Vec::<LinSrgb>::new();

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
                        info!("Reset raytracer with new input");

                        // Update input.
                        input = latest_input;

                        // Reset sampling state.
                        sample_index = 0;
                        pixel_sample_buffer.clear();
                        pixel_sample_buffer.resize(
                            (input.image_size.0 * input.image_size.1) as usize,
                            LinSrgb::default(),
                        );

                        // Reset camera.
                        let camera = &glb_scene.cameras[0];
                        let camera_transform = input.camera_transform.try_inverse().unwrap();
                        let view_from_clip = camera.clip_from_view().inverse();
                        let world_from_view = camera.world_from_view();
                        world_from_clip = camera_transform * world_from_view * view_from_clip;
                        camera_position = camera_transform.transform_point(&camera.position());

                        // Reset stats.
                        ray_stats = intersection::RayBvhHitStats::default();

                        // Reset timer.
                        timer = Instant::now();
                    }
                }

                // If the state is invalid, skip.
                if !input.is_valid() {
                    continue;
                }

                // Render.
                if sample_index < params.samples_per_pixel {
                    let image_size = input.image_size;
                    let pixel_count = image_size.0 * image_size.1;
                    for pixel_index in 0..pixel_count {
                        let pixel_x = pixel_index % image_size.0;
                        let pixel_y = pixel_index / image_size.0;
                        let radiance = radiance(
                            (pixel_x, pixel_y),
                            image_size,
                            camera_position,
                            world_from_clip,
                            &mut rng,
                            uniform_01,
                            input.hemisphere_sampler,
                            &params,
                            &scene,
                            &mut ray_stats,
                            materials,
                            textures,
                        );
                        pixel_sample_buffer[pixel_index as usize] += radiance;
                    }

                    // Normalize the current image, send it.
                    let normalization_factor = 1.0 / (sample_index + 1) as f32;
                    let image = pixel_sample_buffer
                        .clone()
                        .into_iter()
                        .map(|sample| sample * normalization_factor)
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
                        info!(
                            "Rendering took {:.03} s, {:.03} rays/s",
                            elapsed,
                            ray_stats.rays as f64 / elapsed
                        );
                        debug!("Stats: {ray_stats:#?}");
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

fn radiance(
    (pixel_x, pixel_y): (u32, u32),
    image_size: (u32, u32),
    camera_position: na::Point3<f32>,
    world_from_clip: na::Matrix4<f32>,
    rng: &mut rand_pcg::Mcg128Xsl64,
    uniform_01: rand::distributions::Uniform<f32>,
    hemisphere_sampler: sampling::HemisphereSampler,
    params: &Params,
    scene: &Scene,
    ray_stats: &mut intersection::RayBvhHitStats,
    materials: &[glb::Material],
    textures: &[glb::Texture],
) -> LinSrgb {
    let mut ray = {
        sampling::camera_ray_uniform(
            (pixel_x, pixel_y),
            image_size,
            &camera_position,
            &world_from_clip,
            uniform_01.sample(rng),
            uniform_01.sample(rng),
        )
    };
    let mut radiance = LinSrgb::new(0.0, 0.0, 0.0);
    let mut throughput = LinSrgb::new(1.0, 1.0, 1.0);
    for _ in 0..params.max_bounce_count {
        // Hit scene.
        let mut closest_hit = 0.0;
        let mut barycentrics = na::Vector3::zeros();
        let mut triangle_index = 0;
        let found_hit = intersection::ray_bvh_hit(
            &ray,
            &scene.bvh_nodes,
            &scene.triangles,
            &mut closest_hit,
            &mut barycentrics,
            &mut triangle_index,
            ray_stats,
        );

        // Special case: ray hit the sky.
        if !found_hit {
            // Todo: Replace with a proper sky model.
            let sun_direction = na::Vector3::new(1.0, 3.0, 1.0).normalize();
            let sky_factor = 0.5 + 0.5 * sun_direction.dot(&ray.dir);
            radiance += throughput * sky_factor;
            break;
        }

        // Unpack triangle data.
        let triangle = &scene.triangles[triangle_index as usize];
        let tex_coord = {
            na::Point::from(
                triangle.tex_coords[0].coords * barycentrics.x
                    + triangle.tex_coords[1].coords * barycentrics.y
                    + triangle.tex_coords[2].coords * barycentrics.z,
            )
        };
        let normal = {
            triangle.normals[0].into_inner() * barycentrics.x
                + triangle.normals[1].into_inner() * barycentrics.y
                + triangle.normals[2].into_inner() * barycentrics.z
        };
        let material = &materials[triangle.material as usize];
        let texture = &textures[material.base_color as usize];

        // Sample texture.
        let base_color = texture.sample(tex_coord).color;

        // Lambertian BRDF, division of PI is the normalization factor.
        let brdf = base_color / PI;

        // Sample next direction, adjust closest hit to avoid spawning the next ray inside the surface.
        ray.origin += 0.999 * closest_hit * ray.dir.into_inner();
        ray.dir = hemisphere_sampler.dir(&normal, uniform_01.sample(rng), uniform_01.sample(rng));

        // Cos theta, clamp to avoid division with very small number.
        let cos_theta = f32::max(0.001, ray.dir.dot(&normal));

        // Probability density function.
        let pdf = hemisphere_sampler.pdf(cos_theta);

        // Update throughput.
        throughput *= brdf * cos_theta / pdf;
    }
    radiance
}
