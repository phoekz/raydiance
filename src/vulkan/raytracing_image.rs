use super::*;

pub struct RaytracingImage {
    handle: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
}

pub struct RaytracingImageRenderer {
    image: Option<RaytracingImage>,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    sampler: vk::Sampler,
    desc_set_layout: vk::DescriptorSetLayout,
    vertex_shader: Shader,
    fragment_shader: Shader,
    graphics_pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
}

impl RaytracingImageRenderer {
    pub unsafe fn create(device: &Device) -> Result<Self> {
        // Commands.
        let (command_pool, command_buffer) = {
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
            (command_pool, command_buffers[0])
        };

        // Sampler.
        let sampler = device.create_sampler(
            &vk::SamplerCreateInfo::builder()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE),
            None,
        )?;

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

        // Shaders.
        let (vertex_shader, fragment_shader) = (
            Shader::create(
                device,
                include_bytes!("../shaders/spv/raytracing_image.vert"),
            )?,
            Shader::create(
                device,
                include_bytes!("../shaders/spv/raytracing_image.frag"),
            )?,
        );

        // Pipeline.
        let (graphics_pipeline, pipeline_layout) = {
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
                .cull_mode(vk::CullModeFlags::NONE)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

            // Vertex input state.
            let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder();

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
                .depth_test_enable(false)
                .depth_write_enable(false);

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
            let pipeline_layout = device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::builder()
                    .set_layouts(slice::from_ref(&desc_set_layout)),
                None,
            )?;

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

            (graphics_pipeline[0], pipeline_layout)
        };

        Ok(Self {
            image: None,
            command_pool,
            command_buffer,
            sampler,
            desc_set_layout,
            vertex_shader,
            fragment_shader,
            graphics_pipeline,
            pipeline_layout,
        })
    }

    pub unsafe fn update(
        &mut self,
        device: &Device,
        raytracing_image: &[ColorRgb],
        (raytracing_image_width, raytracing_image_height): (u32, u32),
    ) -> Result<()> {
        // Flush pipeline.
        device.device_wait_idle()?;

        // Cleanup previous.
        if let Some(image) = &mut self.image {
            device.destroy_image_view(image.view, None);
            device.destroy_image(image.handle, None);
            device.free_memory(image.memory, None);
            self.image = None;
        }

        // Image.
        let image = device.create_image(
            &vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::R32G32B32_SFLOAT)
                .extent(vk::Extent3D {
                    width: raytracing_image_width,
                    height: raytracing_image_height,
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
        let index =
            device.find_memory_type_index(vk::MemoryPropertyFlags::DEVICE_LOCAL, requirements)?;
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
                .format(vk::Format::R32G32B32_SFLOAT)
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
            12 * raytracing_image.len(),
            bytemuck::cast_slice(raytracing_image),
        )?;

        // Begin command buffer.
        let cmd = self.command_buffer;
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
                width: raytracing_image_width,
                height: raytracing_image_height,
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
            slice::from_ref(
                &vk::SubmitInfo2::builder().command_buffer_infos(slice::from_ref(
                    &vk::CommandBufferSubmitInfo::builder().command_buffer(cmd),
                )),
            ),
            vk::Fence::null(),
        )?;
        device.queue_wait_idle(**device.queue())?;

        // Cleanup.
        staging.destroy(device);

        self.image = Some(RaytracingImage {
            handle: image,
            memory,
            view,
        });

        Ok(())
    }

    pub unsafe fn draw(&self, device: &Device, cmd: vk::CommandBuffer) {
        if let Some(image) = &self.image {
            device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.graphics_pipeline);
            let image_info = *vk::DescriptorImageInfo::builder()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(image.view)
                .sampler(self.sampler);
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
            device.cmd_draw(cmd, 3, 1, 0, 0);
        }
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.free_command_buffers(self.command_pool, slice::from_ref(&self.command_buffer));
        device.destroy_command_pool(self.command_pool, None);
        self.vertex_shader.destroy(device);
        self.fragment_shader.destroy(device);
        device.destroy_pipeline(self.graphics_pipeline, None);
        device.destroy_pipeline_layout(self.pipeline_layout, None);
        device.destroy_descriptor_set_layout(self.desc_set_layout, None);
        device.destroy_sampler(self.sampler, None);
        if let Some(image) = &self.image {
            device.destroy_image_view(image.view, None);
            device.destroy_image(image.handle, None);
            device.free_memory(image.memory, None);
        }
    }
}
