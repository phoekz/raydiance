use super::*;

struct RasterMesh {
    positions: Buffer,
    tex_coords: Buffer,
    normals: Buffer,
    indices: Buffer,
    index_count: u32,
    transform: Mat4,
    texture: u32,
}

impl RasterMesh {
    unsafe fn destroy(&self, device: &Device) {
        self.positions.destroy(device);
        self.tex_coords.destroy(device);
        self.normals.destroy(device);
        self.indices.destroy(device);
    }
}

struct RasterTexture {
    handle: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    sampler: vk::Sampler,
}

impl RasterTexture {
    unsafe fn destroy(&self, device: &Device) {
        device.destroy_image_view(self.view, None);
        device.destroy_image(self.handle, None);
        device.free_memory(self.memory, None);
        device.destroy_sampler(self.sampler, None);
    }
}

#[repr(C)]
#[derive(Zeroable, Pod, Clone, Copy)]
struct PushConstants {
    transform: Mat4,
    base_color: Vec4,
    flags: u32,
}

pub struct RasterScene {
    meshes: Vec<RasterMesh>,
    textures: Vec<RasterTexture>,
    desc_set_layout: vk::DescriptorSetLayout,
    vertex_shader: Shader,
    fragment_shader: Shader,
    graphics_pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    push_constant_stage_flags: vk::ShaderStageFlags,
    clip_from_view: Mat4,
    view_from_world: Mat4,
}

impl RasterScene {
    pub unsafe fn create(device: &Device, glb_scene: &glb::Scene) -> Result<Self> {
        // Todo: Allocating meshes individually will eventually crash due to
        // `max_memory_allocation_count`, which is only 4096 on most NVIDIA
        // hardware. At that point, we need to start packing meshes into a
        // single allocation.
        let meshes = {
            let mut meshes = vec![];
            for glb_mesh in &glb_scene.meshes {
                let positions = glb_mesh.positions.as_ref();
                let tex_coords = glb_mesh.tex_coords.as_ref();
                let normals = glb_mesh.normals.as_ref();
                let triangles = glb_mesh.triangles.as_ref();
                let transform = glb_mesh.transform;
                let texture = glb_scene.materials[glb_mesh.material as usize].base_color;

                meshes.push(RasterMesh {
                    positions: Buffer::create_init(
                        device,
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        positions,
                    )?,
                    tex_coords: Buffer::create_init(
                        device,
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        tex_coords,
                    )?,
                    normals: Buffer::create_init(
                        device,
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        normals,
                    )?,
                    indices: Buffer::create_init(
                        device,
                        vk::BufferUsageFlags::INDEX_BUFFER,
                        triangles,
                    )?,
                    index_count: glb_mesh.index_count(),
                    transform,
                    texture,
                });
            }
            meshes
        };

        // Textures.
        let textures = {
            // Temporary uploader setup.
            let command_pool = device.create_command_pool(
                &vk::CommandPoolCreateInfo::builder()
                    .queue_family_index(device.queue().index())
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?;
            let command_buffers = device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::builder()
                    .command_buffer_count(1)
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY),
            )?;
            let cmd = command_buffers[0];

            // Create textures.
            let mut textures = vec![];
            for glb_texture in &glb_scene.textures {
                // Unpack.
                let (width, height, format, pixels) = match &glb_texture {
                    glb::Texture::Scalar(s) => (1, 1, vk::Format::R32_SFLOAT, slice::from_ref(s)),
                    glb::Texture::Vector2(v) => (1, 1, vk::Format::R32G32_SFLOAT, v.as_ref()),
                    glb::Texture::Vector3(v) => (1, 1, vk::Format::R32G32B32_SFLOAT, v.as_ref()),
                    glb::Texture::Vector4(v) => (1, 1, vk::Format::R32G32B32A32_SFLOAT, v.as_ref()),
                    glb::Texture::Image {
                        width,
                        height,
                        components,
                        pixels,
                    } => {
                        let format = match components {
                            1 => vk::Format::R32_SFLOAT,
                            2 => vk::Format::R32G32_SFLOAT,
                            3 => vk::Format::R32G32B32_SFLOAT,
                            4 => vk::Format::R32G32B32A32_SFLOAT,
                            _ => bail!("Components must be 1..4, got {} instead", components),
                        };
                        (*width, *height, format, pixels.as_ref())
                    }
                };

                // Image.
                let image = device.create_image(
                    &vk::ImageCreateInfo::builder()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(format)
                        .extent(vk::Extent3D {
                            width,
                            height,
                            depth: 1,
                        })
                        .mip_levels(1)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::LINEAR)
                        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
                        .initial_layout(vk::ImageLayout::UNDEFINED)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )?;

                // Allocate memory.
                let requirements = device.get_image_memory_requirements(image);
                let index = device
                    .find_memory_type_index(vk::MemoryPropertyFlags::DEVICE_LOCAL, requirements)?;
                let memory = device.allocate_memory(
                    &vk::MemoryAllocateInfo::builder()
                        .allocation_size(requirements.size)
                        .memory_type_index(index),
                    None,
                )?;
                device.bind_image_memory(image, memory, 0)?;

                // Image view.
                let view = device.create_image_view(
                    &vk::ImageViewCreateInfo::builder()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .image(image)
                        .format(format)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        }),
                    None,
                )?;

                // Staging buffer.
                let staging = Buffer::create(
                    device,
                    vk::BufferUsageFlags::TRANSFER_SRC,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                    glb_texture.byte_count(),
                    bytemuck::cast_slice(pixels),
                )?;

                // Begin command buffer.
                device.begin_command_buffer(
                    cmd,
                    &vk::CommandBufferBeginInfo::builder()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )?;

                // Transition image UNDEFINED -> TRANSFER_DST_OPTIMAL.
                device.image_memory_barrier(
                    cmd,
                    image,
                    vk::PipelineStageFlags2::TOP_OF_PIPE,
                    vk::AccessFlags2::empty(),
                    vk::PipelineStageFlags2::TRANSFER,
                    vk::AccessFlags2::TRANSFER_WRITE,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageAspectFlags::COLOR,
                );

                // Copy staging buffer to device image.
                let region = vk::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_row_length: 0,
                    buffer_image_height: 0,
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
                    image_extent: vk::Extent3D {
                        width,
                        height,
                        depth: 1,
                    },
                };
                device.cmd_copy_buffer_to_image(
                    cmd,
                    *staging,
                    image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    slice::from_ref(&region),
                );

                // Transition TRANSFER_DST_OPTIMAL -> SHADER_READ_ONLY_OPTIMAL.
                device.image_memory_barrier(
                    cmd,
                    image,
                    vk::PipelineStageFlags2::TRANSFER,
                    vk::AccessFlags2::TRANSFER_WRITE,
                    vk::PipelineStageFlags2::FRAGMENT_SHADER,
                    vk::AccessFlags2::SHADER_READ,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    vk::ImageAspectFlags::COLOR,
                );

                // Submit.
                device.end_command_buffer(cmd)?;
                device.queue_submit2(
                    **device.queue(),
                    slice::from_ref(&vk::SubmitInfo2::builder().command_buffer_infos(
                        slice::from_ref(
                            &vk::CommandBufferSubmitInfo::builder().command_buffer(cmd),
                        ),
                    )),
                    vk::Fence::null(),
                )?;
                device.queue_wait_idle(**device.queue())?;

                // Sampler.
                let sampler = device.create_sampler(
                    &vk::SamplerCreateInfo::builder()
                        .mag_filter(vk::Filter::NEAREST)
                        .min_filter(vk::Filter::NEAREST)
                        .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                        .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                        .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
                    None,
                )?;

                // Cleanup.
                staging.destroy(device);

                textures.push(RasterTexture {
                    handle: image,
                    memory,
                    view,
                    sampler,
                });
            }

            // Cleanup.
            device.free_command_buffers(command_pool, slice::from_ref(&cmd));
            device.destroy_command_pool(command_pool, None);

            textures
        };

        // Descriptor set layout.
        let desc_set_layout = {
            let bindings = *vk::DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT);
            device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::builder()
                    .bindings(slice::from_ref(&bindings))
                    .flags(vk::DescriptorSetLayoutCreateFlags::PUSH_DESCRIPTOR_KHR),
                None,
            )?
        };

        // Pipelines.
        let (vertex_shader, fragment_shader) = (
            Shader::create(device, include_bytes!("../shaders/spv/triangle.vert"))?,
            Shader::create(device, include_bytes!("../shaders/spv/triangle.frag"))?,
        );
        let (graphics_pipeline, pipeline_layout, push_constant_stage_flags) = {
            // Stages.
            let entry_point = CStr::from_bytes_with_nul(b"main\0")?;
            let vertex_stage = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(*vertex_shader)
                .name(entry_point);
            let fragment_stage = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(*fragment_shader)
                .name(entry_point);
            let stages = [*vertex_stage, *fragment_stage];

            // Rasterizer.
            let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
                .polygon_mode(vk::PolygonMode::FILL)
                .line_width(1.0)
                .cull_mode(vk::CullModeFlags::BACK)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

            // Vertex input state.
            let vertex_binding_descriptions = [
                *vk::VertexInputBindingDescription::builder()
                    .binding(0)
                    .input_rate(vk::VertexInputRate::VERTEX)
                    .stride((3 * size_of::<f32>()) as u32),
                *vk::VertexInputBindingDescription::builder()
                    .binding(1)
                    .input_rate(vk::VertexInputRate::VERTEX)
                    .stride((2 * size_of::<f32>()) as u32),
                *vk::VertexInputBindingDescription::builder()
                    .binding(2)
                    .input_rate(vk::VertexInputRate::VERTEX)
                    .stride((3 * size_of::<f32>()) as u32),
            ];
            let vertex_attribute_descriptions = [
                *vk::VertexInputAttributeDescription::builder()
                    .binding(0)
                    .location(0)
                    .format(vk::Format::R32G32B32_SFLOAT),
                *vk::VertexInputAttributeDescription::builder()
                    .binding(1)
                    .location(1)
                    .format(vk::Format::R32G32_SFLOAT),
                *vk::VertexInputAttributeDescription::builder()
                    .binding(2)
                    .location(2)
                    .format(vk::Format::R32G32B32_SFLOAT),
            ];
            let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
                .vertex_binding_descriptions(&vertex_binding_descriptions)
                .vertex_attribute_descriptions(&vertex_attribute_descriptions);

            // Input assembly state.
            let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

            // Dynamic state.
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
                .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);

            // Viewport state.
            let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
                .viewport_count(1)
                .scissor_count(1);

            // Depth stencil state.
            let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::builder()
                .depth_test_enable(true)
                .depth_write_enable(true)
                .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
                .back(*vk::StencilOpState::builder().compare_op(vk::CompareOp::ALWAYS));

            // Color blend state.
            let color_blend_attachment = vk::PipelineColorBlendAttachmentState::builder()
                .blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(vk::BlendFactor::ONE)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE)
                .alpha_blend_op(vk::BlendOp::ADD)
                .color_write_mask(vk::ColorComponentFlags::RGBA);
            let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
                .attachments(slice::from_ref(&color_blend_attachment));

            // Multisample state.
            let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
                .rasterization_samples(DEFAULT_SAMPLE_COUNT);

            // Rendering.
            let mut rendering = vk::PipelineRenderingCreateInfo::builder()
                .color_attachment_formats(&[DEFAULT_SURFACE_FORMAT])
                .depth_attachment_format(DEFAULT_DEPTH_FORMAT);

            // Pipeline layout.
            let push_constant_stage_flags =
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            let pipeline_layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::builder()
                        .set_layouts(slice::from_ref(&desc_set_layout))
                        .push_constant_ranges(&[vk::PushConstantRange {
                            stage_flags: push_constant_stage_flags,
                            offset: 0,
                            size: size_of::<PushConstants>() as u32,
                        }]),
                    None,
                )
                .context("Creating pipeline layout")?;

            // Pipeline.
            let graphics_pipeline = device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    slice::from_ref(
                        &vk::GraphicsPipelineCreateInfo::builder()
                            .stages(&stages)
                            .rasterization_state(&rasterization_state)
                            .vertex_input_state(&vertex_input_state)
                            .input_assembly_state(&input_assembly_state)
                            .dynamic_state(&dynamic_state)
                            .viewport_state(&viewport_state)
                            .depth_stencil_state(&depth_stencil_state)
                            .color_blend_state(&color_blend_state)
                            .multisample_state(&multisample_state)
                            .push_next(&mut rendering)
                            .layout(pipeline_layout),
                    ),
                    None,
                )
                .unwrap();

            (
                graphics_pipeline[0],
                pipeline_layout,
                push_constant_stage_flags,
            )
        };

        // Camera.
        let (clip_from_view, view_from_world) = {
            let camera = &glb_scene.cameras[0];
            (
                *camera.clip_from_view().as_matrix(),
                camera.world_from_view().try_inverse().unwrap(),
            )
        };

        Ok(Self {
            meshes,
            textures,
            desc_set_layout,
            vertex_shader,
            fragment_shader,
            graphics_pipeline,
            pipeline_layout,
            push_constant_stage_flags,
            clip_from_view,
            view_from_world,
        })
    }

    pub unsafe fn draw(
        &self,
        device: &Device,
        cmd: vk::CommandBuffer,
        camera_transform: Mat4,
        dyn_scene: &glb::DynamicScene,
        visualize_normals: bool,
    ) {
        // Prepare matrices.
        let clip_from_view = self.clip_from_view;
        let view_from_world = self.view_from_world;

        // Render meshes.
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.graphics_pipeline);
        for mesh in &self.meshes {
            // Prepare push constants.
            let base_color = match glb::dynamic_try_sample(dyn_scene, mesh.texture) {
                Some(v) => transmute(v),
                None => vector![0.0, 0.0, 0.0, 0.0],
            };
            let push = PushConstants {
                // Pre-multiply all matrices to save space.
                // `max_push_constants_size` is typically in order of 128 to 256
                // bytes.
                transform: clip_from_view * view_from_world * camera_transform * mesh.transform,
                base_color,
                flags: u32::from(visualize_normals),
            };
            let constants = bytemuck::cast_slice(slice::from_ref(&push));

            // Bind resources.
            device.cmd_bind_vertex_buffers(cmd, 0, slice::from_ref(&*mesh.positions), &[0]);
            device.cmd_bind_vertex_buffers(cmd, 1, slice::from_ref(&*mesh.tex_coords), &[0]);
            device.cmd_bind_vertex_buffers(cmd, 2, slice::from_ref(&*mesh.normals), &[0]);
            device.cmd_bind_index_buffer(cmd, *mesh.indices, 0, vk::IndexType::UINT32);
            device.cmd_push_constants(
                cmd,
                self.pipeline_layout,
                self.push_constant_stage_flags,
                0,
                constants,
            );

            // Bind texture and sampler.
            {
                let texture = &self.textures[mesh.texture as usize];
                let image_info = *vk::DescriptorImageInfo::builder()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(texture.view)
                    .sampler(texture.sampler);
                let write = *vk::WriteDescriptorSet::builder()
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(slice::from_ref(&image_info));
                device.push_descriptor_khr().cmd_push_descriptor_set(
                    cmd,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline_layout,
                    0,
                    slice::from_ref(&write),
                );
            }

            // Draw.
            device.cmd_draw_indexed(cmd, mesh.index_count, 1, 0, 0, 0);
        }
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.destroy_descriptor_set_layout(self.desc_set_layout, None);
        self.vertex_shader.destroy(device);
        self.fragment_shader.destroy(device);
        for mesh in &self.meshes {
            mesh.destroy(device);
        }
        for texture in &self.textures {
            texture.destroy(device);
        }
        device.destroy_pipeline(self.graphics_pipeline, None);
        device.destroy_pipeline_layout(self.pipeline_layout, None);
    }
}
