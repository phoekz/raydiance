use super::*;

use vulk::vk;
use vulk_ext::vkx::{self, prelude::*};

pub struct RendererCreateInfo {
    pub image_size: (u32, u32),
    pub rds_scene: rds::Scene,
}

pub struct Renderer {
    scene: rds::Scene,
    instance: vkx::Instance,
    _physical_device: vkx::PhysicalDevice,
    device: vkx::Device,
    command_buffer: vkx::CommandBuffer,
    command_buffer_done: vkx::TimelineSemaphore,
    timestamps: vkx::TimestampQuery,
    statistics: vkx::StatisticsQuery,
    color_target: vkx::ImageDedicatedResource,
    depth_target: vkx::ImageDedicatedResource,
    resolve_target: vkx::ImageDedicatedResource,
    output_buffer: vkx::BufferDedicatedTransfer,
    shader: vkx::Shader,
    clip_from_view: Mat4,
    view_from_world: Mat4,
    descriptor_storage: vkx::DescriptorStorage,
    buffer_resources: Vec<vkx::BufferResource>,
    buffer_allocations: vkx::BufferAllocations,
    image_resources: Vec<vkx::ImageResource>,
    image_allocations: vkx::ImageAllocations,
    texture_flags_buffer: vkx::BufferResource,
    dynamic_textures_buffer: vkx::BufferResource,
    texture_buffer_allocations: vkx::BufferAllocations,
    sampler: vkx::SamplerResource,
}

pub struct RendererInput {
    pub frame_index: u64,
    pub frame_count: u64,
    pub camera_transform: Mat4,
    pub image_size: (u32, u32),
    pub dyn_scene: rds::DynamicScene,
}

#[repr(C)]
#[derive(Debug)]
struct Mesh {
    transform: Mat4,
    material: u32,
    triangle_count: u32,
}

#[repr(C)]
#[derive(Debug)]
struct Material {
    base_color: u32,
    metallic: u32,
    roughness: u32,
    specular: u32,
    specular_tint: u32,
    sheen: u32,
    sheen_tint: u32,
}

#[repr(C)]
#[derive(Debug)]
struct DynamicMaterial {
    base_color: Vec4,
    metallic: f32,
    roughness: f32,
    specular: f32,
    specular_tint: f32,
    sheen: f32,
    sheen_tint: f32,
    replaced_mask: u32,
}

#[repr(C)]
#[derive(Debug)]
struct PushConstants {
    transform: Mat4,
}

const COLOR_TARGET_FORMAT: vk::Format = vk::Format::R8g8b8a8Unorm;
const DEPTH_TARGET_FORMAT: vk::Format = vk::Format::D32Sfloat;
const RESOLVE_TARGET_FORMAT: vk::Format = vk::Format::R8g8b8a8Unorm;

const SAMPLE_COUNT: vk::SampleCountFlagBits = vk::SampleCountFlagBits::Count8;

impl Renderer {
    pub fn create(create_info: RendererCreateInfo) -> Result<Self> {
        unsafe {
            // Basic machinery.
            let instance = vkx::Instance::create(&vkx::InstanceCreateInfo {
                validation_layers: true,
                ..Default::default()
            })?;
            let physical_device = vkx::PhysicalDevice::create(&instance)?;
            let device = vkx::Device::create(&instance, &physical_device, None)?;
            let command_buffer = vkx::CommandBuffer::create(&device)?;
            let command_buffer_done = vkx::TimelineSemaphore::create(&device, 0)?;
            let timestamps = vkx::TimestampQuery::create(&physical_device, &device, 2)?;
            let statistics = vkx::StatisticsQuery::create(&device)?;
            let color_target = vkx::ImageDedicatedResource::create_2d(
                &physical_device,
                &device,
                vkx::ImageCreator::new_2d_samples(
                    create_info.image_size.0,
                    create_info.image_size.1,
                    COLOR_TARGET_FORMAT,
                    vk::ImageUsageFlagBits::InputAttachment
                        | vk::ImageUsageFlagBits::ColorAttachment,
                    SAMPLE_COUNT,
                ),
                vk::MemoryPropertyFlagBits::DeviceLocal,
            )?;
            let depth_target = vkx::ImageDedicatedResource::create_2d(
                &physical_device,
                &device,
                vkx::ImageCreator::new_2d_samples(
                    create_info.image_size.0,
                    create_info.image_size.1,
                    DEPTH_TARGET_FORMAT,
                    vk::ImageUsageFlagBits::InputAttachment
                        | vk::ImageUsageFlagBits::DepthStencilAttachment,
                    SAMPLE_COUNT,
                ),
                vk::MemoryPropertyFlagBits::DeviceLocal,
            )?;
            let resolve_target = vkx::ImageDedicatedResource::create_2d(
                &physical_device,
                &device,
                vkx::ImageCreator::new_2d_samples(
                    create_info.image_size.0,
                    create_info.image_size.1,
                    RESOLVE_TARGET_FORMAT,
                    vk::ImageUsageFlagBits::InputAttachment
                        | vk::ImageUsageFlagBits::ColorAttachment
                        | vk::ImageUsageFlagBits::TransferSrc,
                    vk::SampleCountFlagBits::Count1,
                ),
                vk::MemoryPropertyFlagBits::DeviceLocal,
            )?;
            let output_buffer = vkx::BufferDedicatedTransfer::create(
                &physical_device,
                &device,
                vkx::BufferCreator::new(
                    resolve_target.byte_size(),
                    vk::BufferUsageFlagBits::TransferDst,
                ),
                vk::MemoryPropertyFlagBits::HostVisible,
            )?;

            // Camera.
            let (clip_from_view, view_from_world) = {
                let camera = &create_info.rds_scene.cameras[0];
                (
                    *camera.clip_from_view().as_matrix(),
                    camera.world_from_view().try_inverse().unwrap(),
                )
            };

            // Buffer usage.
            let buffer_usage =
                vk::BufferUsageFlagBits::StorageBuffer | vk::BufferUsageFlagBits::TransferDst;

            // Mesh data.
            let mut mesh_buffer = vec![];
            for mesh in &create_info.rds_scene.meshes {
                mesh_buffer.push(Mesh {
                    transform: mesh.transform,
                    material: mesh.material,
                    triangle_count: mesh.triangle_count(),
                });
            }
            let (mesh_buffer_creator, mesh_buffer_bytes) = {
                let size = size_of::<Mesh>() * mesh_buffer.len();
                let ptr: *const u8 = mesh_buffer.as_ptr().cast();
                (
                    vkx::BufferCreator::new(size as _, buffer_usage),
                    slice::from_raw_parts(ptr, size),
                )
            };

            // Material data.
            let mut material_buffer = vec![];
            for material in &create_info.rds_scene.materials {
                material_buffer.push(Material {
                    base_color: material.base_color,
                    metallic: material.metallic,
                    roughness: material.roughness,
                    specular: material.specular,
                    specular_tint: material.specular_tint,
                    sheen: material.sheen,
                    sheen_tint: material.sheen_tint,
                });
            }
            let (material_buffer_creator, material_buffer_bytes) = {
                let size = size_of::<Material>() * material_buffer.len();
                let ptr: *const u8 = material_buffer.as_ptr().cast();
                (
                    vkx::BufferCreator::new(size as _, buffer_usage),
                    slice::from_raw_parts(ptr, size),
                )
            };

            // Buffers.
            let mut buffer_creators = vec![];
            let mut buffer_bytes = vec![];
            buffer_creators.push(mesh_buffer_creator);
            buffer_bytes.push(mesh_buffer_bytes);
            buffer_creators.push(material_buffer_creator);
            buffer_bytes.push(material_buffer_bytes);
            for mesh in &create_info.rds_scene.meshes {
                let size = size_of::<Vec3>() * mesh.positions.len();
                let ptr: *const u8 = mesh.positions.as_ptr().cast();
                buffer_creators.push(vkx::BufferCreator::new(size as _, buffer_usage));
                buffer_bytes.push(slice::from_raw_parts(ptr, size));

                let size = size_of::<Vec2>() * mesh.tex_coords.len();
                let ptr: *const u8 = mesh.tex_coords.as_ptr().cast();
                buffer_creators.push(vkx::BufferCreator::new(size as _, buffer_usage));
                buffer_bytes.push(slice::from_raw_parts(ptr, size));

                let size = size_of::<Normal>() * mesh.normals.len();
                let ptr: *const u8 = mesh.normals.as_ptr().cast();
                buffer_creators.push(vkx::BufferCreator::new(size as _, buffer_usage));
                buffer_bytes.push(slice::from_raw_parts(ptr, size));

                let size = size_of::<Vec3u>() * mesh.triangles.len();
                let ptr: *const u8 = mesh.triangles.as_ptr().cast();
                buffer_creators.push(vkx::BufferCreator::new(size as _, buffer_usage));
                buffer_bytes.push(slice::from_raw_parts(ptr, size));
            }
            let (buffer_resources, buffer_allocations) = vkx::BufferResource::create(
                &physical_device,
                &device,
                &buffer_creators,
                vk::MemoryPropertyFlagBits::DeviceLocal,
            )?;
            vkx::transfer_resources(
                &physical_device,
                &device,
                &buffer_resources,
                &buffer_bytes,
                &[],
                &[],
            )?;

            // Images.
            let mut image_creators = vec![];
            let mut image_bytes = vec![];
            for texture in &create_info.rds_scene.textures {
                // Canonicalize.
                let (width, height, format, pixels) = match &texture {
                    rds::Texture::Scalar(s) => (1, 1, vk::Format::R32Sfloat, slice::from_ref(s)),
                    rds::Texture::Vector2(v) => (1, 1, vk::Format::R32g32Sfloat, v.as_ref()),
                    rds::Texture::Vector3(v) => (1, 1, vk::Format::R32g32b32Sfloat, v.as_ref()),
                    rds::Texture::Vector4(v) => (1, 1, vk::Format::R32g32b32a32Sfloat, v.as_ref()),
                    rds::Texture::Image {
                        width,
                        height,
                        components,
                        pixels,
                    } => {
                        let format = match components {
                            1 => vk::Format::R32Sfloat,
                            2 => vk::Format::R32g32Sfloat,
                            3 => vk::Format::R32g32b32Sfloat,
                            4 => vk::Format::R32g32b32a32Sfloat,
                            _ => bail!("Components must be 1..4, got {} instead", components),
                        };
                        (*width, *height, format, pixels.as_ref())
                    }
                };
                image_creators.push(vkx::ImageCreator::new_2d(
                    width,
                    height,
                    format,
                    vk::ImageUsageFlagBits::TransferDst | vk::ImageUsageFlagBits::Sampled,
                ));
                image_bytes.push(slice::from_raw_parts(
                    pixels.as_ptr().cast(),
                    size_of::<f32>() * pixels.len(),
                ));
            }
            let (image_resources, image_allocations) = vkx::ImageResource::create(
                &physical_device,
                &device,
                &image_creators,
                vk::MemoryPropertyFlagBits::DeviceLocal,
            )?;
            vkx::transfer_resources(
                &physical_device,
                &device,
                &[],
                &[],
                &image_resources,
                &image_bytes,
            )?;

            // Texture flags / dynamic textures.
            let (mut texture_buffer_resources, texture_buffer_allocations) =
                vkx::BufferResource::create(
                    &physical_device,
                    &device,
                    &[
                        vkx::BufferCreator::new(
                            (size_of::<u32>() * create_info.rds_scene.textures.len()) as _,
                            vk::BufferUsageFlagBits::StorageBuffer,
                        ),
                        vkx::BufferCreator::new(
                            (size_of::<Vec4>() * create_info.rds_scene.textures.len()) as _,
                            vk::BufferUsageFlagBits::StorageBuffer,
                        ),
                    ],
                    vk::MemoryPropertyFlagBits::HostVisible
                        | vk::MemoryPropertyFlagBits::HostCoherent,
                )?;

            // Initialize texture buffers.
            let mut texture_flags_buffer = texture_buffer_resources.remove(0);
            texture_flags_buffer
                .memory_mut()
                .as_mut_slice(create_info.rds_scene.textures.len())
                .fill(0_u32);
            let texture_flags_descriptor = texture_flags_buffer.descriptor();

            let mut dynamic_textures_buffer = texture_buffer_resources.remove(0);
            dynamic_textures_buffer
                .memory_mut()
                .as_mut_slice(create_info.rds_scene.textures.len())
                .fill(Vec4::zeros());
            let dynamic_textures_descriptor = dynamic_textures_buffer.descriptor();

            // Sampler.
            let sampler_creator = vkx::SamplerCreator::new()
                .mag_filter(vk::Filter::Nearest)
                .min_filter(vk::Filter::Nearest)
                .mipmap_mode(vk::SamplerMipmapMode::Nearest)
                .address_mode_uvw(vk::SamplerAddressMode::ClampToEdge);
            let (sampler, sampler_create_info) = sampler_creator.create(&device)?;
            let mut samplers = vkx::SamplerResource::create(
                &physical_device,
                &device,
                &[sampler],
                &[sampler_create_info],
            )?;
            let sampler = samplers.remove(0);
            let sampler_descriptor = sampler.descriptor();

            // Stages.
            let stage_flags = vk::ShaderStageFlagBits::TaskEXT
                | vk::ShaderStageFlagBits::MeshEXT
                | vk::ShaderStageFlagBits::Fragment;

            // Descriptor storage.
            let meshes_descriptor = buffer_resources[0].descriptor();
            let materials_descriptor = buffer_resources[1].descriptor();
            let bindings_count = 4;
            let mut positions_descriptors = vec![];
            let mut tex_coords_descriptors = vec![];
            let mut normals_descriptors = vec![];
            let mut triangles_descriptors = vec![];
            for buffer_resources in buffer_resources[2..].chunks_exact(bindings_count) {
                positions_descriptors.push(buffer_resources[0].descriptor());
                tex_coords_descriptors.push(buffer_resources[1].descriptor());
                normals_descriptors.push(buffer_resources[2].descriptor());
                triangles_descriptors.push(buffer_resources[3].descriptor());
            }
            let image_descriptors = image_resources
                .iter()
                .map(vkx::ImageResource::descriptor)
                .collect::<Vec<_>>();
            let bindings = vec![
                // binding = 0
                vkx::DescriptorBinding {
                    ty: vk::DescriptorType::StorageBuffer,
                    stages: stage_flags,
                    descriptors: slice::from_ref(&meshes_descriptor),
                },
                // binding = 1
                vkx::DescriptorBinding {
                    ty: vk::DescriptorType::StorageBuffer,
                    stages: stage_flags,
                    descriptors: &positions_descriptors,
                },
                // binding = 2
                vkx::DescriptorBinding {
                    ty: vk::DescriptorType::StorageBuffer,
                    stages: stage_flags,
                    descriptors: &tex_coords_descriptors,
                },
                // binding = 3
                vkx::DescriptorBinding {
                    ty: vk::DescriptorType::StorageBuffer,
                    stages: stage_flags,
                    descriptors: &normals_descriptors,
                },
                // binding = 4
                vkx::DescriptorBinding {
                    ty: vk::DescriptorType::StorageBuffer,
                    stages: stage_flags,
                    descriptors: &triangles_descriptors,
                },
                // binding = 5
                vkx::DescriptorBinding {
                    ty: vk::DescriptorType::StorageBuffer,
                    stages: stage_flags,
                    descriptors: slice::from_ref(&materials_descriptor),
                },
                // binding = 6
                vkx::DescriptorBinding {
                    ty: vk::DescriptorType::SampledImage,
                    stages: stage_flags,
                    descriptors: &image_descriptors,
                },
                // binding = 7
                vkx::DescriptorBinding {
                    ty: vk::DescriptorType::StorageBuffer,
                    stages: stage_flags,
                    descriptors: slice::from_ref(&texture_flags_descriptor),
                },
                // binding = 8
                vkx::DescriptorBinding {
                    ty: vk::DescriptorType::StorageBuffer,
                    stages: stage_flags,
                    descriptors: slice::from_ref(&dynamic_textures_descriptor),
                },
                // binding = 9
                vkx::DescriptorBinding {
                    ty: vk::DescriptorType::Sampler,
                    stages: stage_flags,
                    descriptors: slice::from_ref(&sampler_descriptor),
                },
            ];
            let descriptor_storage = vkx::DescriptorStorage::create(
                &physical_device,
                &device,
                &bindings,
                Some(vk::PushConstantRange {
                    stage_flags,
                    offset: 0,
                    size: size_of::<PushConstants>() as _,
                }),
            )?;

            // Shaders.
            let mut compiler = vkx::ShaderCompiler::new()?;
            compiler.include("common.glsl", include_str!("shaders/common.glsl"));
            let task_binary = compiler.compile(
                vkx::ShaderType::Task,
                "triangle.task.glsl",
                "main",
                include_str!("shaders/triangle.task.glsl"),
            )?;
            let mesh_binary = compiler.compile(
                vkx::ShaderType::Mesh,
                "triangle.mesh.glsl",
                "main",
                include_str!("shaders/triangle.mesh.glsl"),
            )?;
            let frag_binary = compiler.compile(
                vkx::ShaderType::Fragment,
                "triangle.frag.glsl",
                "main",
                include_str!("shaders/triangle.frag.glsl"),
            )?;
            let shader = vkx::Shader::create(
                &device,
                &vkx::ShaderCreateInfo {
                    shader_binaries: &[task_binary, mesh_binary, frag_binary],
                    set_layouts: descriptor_storage.set_layouts(),
                    push_constant_ranges: descriptor_storage.push_constant_ranges(),
                    specialization_info: None,
                },
            )?;

            Ok(Self {
                scene: create_info.rds_scene,
                instance,
                _physical_device: physical_device,
                device,
                command_buffer,
                command_buffer_done,
                timestamps,
                statistics,
                color_target,
                depth_target,
                resolve_target,
                output_buffer,
                shader,
                clip_from_view,
                view_from_world,
                descriptor_storage,
                buffer_resources,
                buffer_allocations,
                image_resources,
                image_allocations,
                texture_flags_buffer,
                dynamic_textures_buffer,
                texture_buffer_allocations,
                sampler,
            })
        }
    }

    pub fn destroy(self) {
        unsafe {
            self.command_buffer.destroy(&self.device);
            self.command_buffer_done.destroy(&self.device);
            self.timestamps.destroy(&self.device);
            self.statistics.destroy(&self.device);
            self.color_target.destroy(&self.device);
            self.depth_target.destroy(&self.device);
            self.resolve_target.destroy(&self.device);
            self.output_buffer.destroy(&self.device);
            self.shader.destroy(&self.device);
            self.descriptor_storage.destroy(&self.device);
            for buffer_resource in self.buffer_resources {
                buffer_resource.destroy(&self.device);
            }
            self.buffer_allocations.free(&self.device);
            for image_resource in self.image_resources {
                image_resource.destroy(&self.device);
            }
            self.image_allocations.free(&self.device);
            self.texture_flags_buffer.destroy(&self.device);
            self.dynamic_textures_buffer.destroy(&self.device);
            self.texture_buffer_allocations.free(&self.device);
            self.sampler.destroy(&self.device);
            self.device.destroy();
            self.instance.destroy();
        }
    }

    pub fn update(&mut self, input: &RendererInput) {
        unsafe {
            let texture_count = self.scene.textures.len();
            let mut texture_flags = self
                .texture_flags_buffer
                .memory_mut()
                .as_mut_slice::<u32>(texture_count);
            let mut dynamic_textures = self
                .dynamic_textures_buffer
                .memory_mut()
                .as_mut_slice::<Vec4>(texture_count);

            for texture_id in 0..texture_count {
                if let Some(v) = rds::dynamic_try_sample(&input.dyn_scene, texture_id as _) {
                    texture_flags[texture_id] = 1;
                    dynamic_textures[texture_id] = transmute(v);
                } else {
                    texture_flags[texture_id] = 0;
                    dynamic_textures[texture_id] = Vec4::new(0.0, 0.0, 0.0, 0.0);
                };
            }
        }

        //
    }

    pub fn render(&self, input: &RendererInput) -> Result<vz::image::Rgb> {
        let Self {
            scene,
            device,
            command_buffer,
            command_buffer_done,
            timestamps,
            statistics,
            color_target,
            depth_target,
            resolve_target,
            output_buffer,
            shader,
            clip_from_view,
            view_from_world,
            descriptor_storage,
            ..
        } = self;

        unsafe {
            // Reset queries.
            timestamps.reset(device);
            statistics.reset(device);

            // Record commands.
            command_buffer.begin(device)?;
            command_buffer.write_timestamp(device, timestamps, 0);
            command_buffer.begin_statistics(device, statistics);
            command_buffer.image_barrier(
                device,
                resolve_target,
                vk::PipelineStageFlagBits2::None,
                vk::AccessFlags2::empty(),
                vk::PipelineStageFlagBits2::ColorAttachmentOutput,
                vk::AccessFlagBits2::ColorAttachmentWrite,
                vk::ImageLayout::Undefined,
                vk::ImageLayout::AttachmentOptimal,
            );
            let clear_color = {
                use palette::{FromColor, Hsl, Srgb};
                let hue = (input.frame_index as f32 + 0.5) / input.frame_count as f32;
                let hsl = Hsl::new(360.0 * hue, 0.75, 0.75);
                let rgb = Srgb::from_color(hsl);
                [rgb.red, rgb.green, rgb.blue, 1.0]
            };
            command_buffer.begin_rendering(
                device,
                (color_target, clear_color),
                Some((depth_target, 1.0)),
                Some(resolve_target),
            );
            command_buffer.set_cull_mode(device, vk::CullModeFlagBits::None);
            command_buffer.set_front_face(device, vk::FrontFace::Clockwise);
            command_buffer.set_depth_test(device, true);
            command_buffer.set_depth_write(device, true);
            command_buffer.set_depth_compare_op(device, vk::CompareOp::Less);
            command_buffer.set_samples(device, SAMPLE_COUNT);
            command_buffer.set_viewport_flip_y(
                device,
                &vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: resolve_target.width() as f32,
                    height: resolve_target.height() as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                },
            );
            command_buffer.bind_descriptor_storage(
                device,
                descriptor_storage,
                vk::PipelineBindPoint::Graphics,
            );
            command_buffer.bind_shader(device, shader);
            command_buffer.push_constants(
                device,
                descriptor_storage,
                &(PushConstants {
                    transform: clip_from_view * view_from_world * input.camera_transform,
                }),
            )?;
            command_buffer.draw_mesh_tasks(device, scene.meshes.len() as _, 1, 1);
            command_buffer.end_rendering(device);
            command_buffer.image_barrier(
                device,
                resolve_target,
                vk::PipelineStageFlagBits2::ColorAttachmentOutput,
                vk::AccessFlagBits2::ColorAttachmentWrite,
                vk::PipelineStageFlagBits2::Copy,
                vk::AccessFlagBits2::TransferRead,
                vk::ImageLayout::AttachmentOptimal,
                vk::ImageLayout::TransferSrcOptimal,
            );
            command_buffer.copy_image_to_buffer(device, resolve_target, (output_buffer, 0));
            command_buffer.end_statistics(device, statistics);
            command_buffer.write_timestamp(device, timestamps, 1);
            command_buffer.end(device)?;

            // Submit & wait.
            let wait_value = input.frame_index + 1;
            vkx::queue_submit(
                device,
                command_buffer,
                &[],
                &[command_buffer_done
                    .submit_info(wait_value, vk::PipelineStageFlagBits2::AllCommands)],
            )?;
            command_buffer_done.wait(device, wait_value, u64::MAX)?;

            // Queries.
            let _timestamp_differences = timestamps.get_differences(device)?[0];
            let statistics = statistics.get_statistics(device)?;
            let mesh_primitives_generated = statistics
                .mesh_primitives_generated
                .mesh_primitives_generated;
            let scene_primitive_count = scene
                .meshes
                .iter()
                .map(|mesh| u64::from(mesh.triangle_count()))
                .sum::<u64>();
            ensure!(mesh_primitives_generated == scene_primitive_count);

            // Copy to frame.
            let width = resolve_target.width();
            let height = resolve_target.height();
            let byte_size = resolve_target.byte_size();
            let mut pixels = vec![0_u8; byte_size as usize];
            pixels.copy_from_slice(output_buffer.memory().as_slice(byte_size as _));
            Ok(imagelib::RgbaImage::from_raw(width, height, pixels)
                .unwrap()
                .into())
        }
    }
}
