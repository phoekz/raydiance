use super::*;

pub struct Scene {
    bvh_nodes: Vec<bvh::Node>,
    triangles: Vec<Triangle>,
}

impl Scene {
    pub fn create(assets_scene: &glb::Scene) -> Self {
        let max_triangle_count = assets_scene
            .meshes
            .iter()
            .map(glb::Mesh::triangle_count)
            .sum::<u32>();
        let mut triangles = Vec::with_capacity(max_triangle_count as usize);
        for mesh in &assets_scene.meshes {
            for triangle in mesh.triangles.iter() {
                let position_0 = mesh.positions[triangle[0] as usize];
                let position_1 = mesh.positions[triangle[1] as usize];
                let position_2 = mesh.positions[triangle[2] as usize];
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

pub struct RenderParameters {
    pub samples_per_pixel: u32,
    pub max_bounce_count: u32,
    pub seed: u64,
}

impl Default for RenderParameters {
    fn default() -> Self {
        Self {
            samples_per_pixel: 64,
            max_bounce_count: 4,
            seed: 0,
        }
    }
}

pub fn render(
    params: &RenderParameters,
    scene: &Scene,
    camera: &glb::Camera,
    materials: &[glb::Material],
    image_size: (u32, u32),
) -> Vec<LinSrgb> {
    // Camera.
    let (world_from_clip, camera_position) = {
        let camera_position = camera.position();
        let view_from_clip = camera.clip_from_view().inverse();
        let world_from_view = camera.world_from_view();
        (world_from_view * view_from_clip, camera_position)
    };

    // Path tracing.
    let image = {
        let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(params.seed);
        let uniform_01 = rand::distributions::Uniform::new_inclusive(0.0_f32, 1.0_f32);
        let pixel_positions = {
            let mut pixels = Vec::with_capacity((image_size.0 * image_size.1) as usize);
            for y in 0..image_size.1 {
                for x in 0..image_size.0 {
                    pixels.push((x, y));
                }
            }
            pixels
        };

        let timer = Instant::now();
        let mut ray_stats = intersection::RayBvhHitStats::default();
        let image = pixel_positions
            .iter()
            .map(|&(pixel_x, pixel_y)| {
                let mut sample_color = LinSrgb::new(0.0, 0.0, 0.0);
                for _ in 0..params.samples_per_pixel {
                    let mut ray = sampling::camera_ray_uniform(
                        (pixel_x, pixel_y),
                        image_size,
                        &camera_position,
                        &world_from_clip,
                        uniform_01.sample(&mut rng),
                        uniform_01.sample(&mut rng),
                    );
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
                            &mut ray_stats,
                        );

                        // Special case: ray hit the sky.
                        if !found_hit {
                            // Todo: Replace with a proper sky model.
                            let sun_direction = na::Vector3::new(1.0, 2.0, 1.0).normalize();
                            let sky_factor = 0.75 + 0.25 * sun_direction.dot(&ray.dir);
                            radiance += throughput * sky_factor;
                            break;
                        }

                        // Unpack triangle data.
                        let triangle = &scene.triangles[triangle_index as usize];
                        let normal = {
                            triangle.normals[0].into_inner() * barycentrics.x
                                + triangle.normals[1].into_inner() * barycentrics.y
                                + triangle.normals[2].into_inner() * barycentrics.z
                        };
                        let material = &materials[triangle.material as usize];

                        // Lambertian BRDF, division of PI is the normalization factor.
                        let brdf = material.base_color / PI;

                        // Sample next direction, adjust closest hit to avoid spawning the next ray inside the surface.
                        ray.origin += 0.999 * closest_hit * ray.dir.into_inner();
                        ray.dir = sampling::direction_uniform(
                            &normal,
                            uniform_01.sample(&mut rng),
                            uniform_01.sample(&mut rng),
                        );

                        // Cos theta, clamp to avoid division with very small number.
                        let cos_theta = f32::max(0.001, ray.dir.dot(&normal));

                        // PDF for uniformly sampled hemisphere.
                        let pdf = 1.0 / (2.0 * PI);

                        // Update throughput.
                        throughput *= brdf * cos_theta / pdf;
                    }

                    // Accumulate samples.
                    sample_color += radiance;
                }
                // Average samples.
                sample_color / (params.samples_per_pixel as f32)
            })
            .collect::<Vec<_>>();

        debug!("Stats: {ray_stats:#?}");

        {
            let elapsed = timer.elapsed().as_secs_f64();
            info!(
                "Rendering took {:.03} s, {:.03} rays/s",
                elapsed,
                ray_stats.rays as f64 / elapsed
            );
        }

        image
    };

    image
}
