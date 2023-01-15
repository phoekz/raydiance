use super::*;

pub struct Gui {
    geometry: Vec<Geometry>,
    font_atlas: FontAtlas,
    sampler: vk::Sampler,
    desc_set_layout: vk::DescriptorSetLayout,
    vertex_shader: Shader,
    fragment_shader: Shader,
    graphics_pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
}

struct Geometry {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
}

struct FontAtlas {
    handle: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
}

#[repr(C)]
#[derive(Zeroable, Pod, Clone, Copy)]
struct PushConstants {
    scale: na::Vector2<f32>,
    translation: na::Vector2<f32>,
}

impl Gui {
    const MAX_VERTEX_COUNT: usize = 128 * 1024;
    const MAX_INDEX_COUNT: usize = 3 * Self::MAX_VERTEX_COUNT;

    pub unsafe fn create(
        device: &Device,
        font_atlas_texture: &imgui::FontAtlasTexture,
    ) -> Result<Self> {
        // Allocate buffers.
        let vertex_size = size_of::<imgui::DrawVert>();
        let index_size = size_of::<imgui::DrawIdx>();
        assert!(vertex_size == 20);
        assert!(index_size == size_of::<u16>());
        let mut geometry = vec![];
        for _ in 0..MAX_CONCURRENT_FRAMES {
            let vertex_buffer = Buffer::create(
                device,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                vertex_size * Self::MAX_VERTEX_COUNT,
                &[],
            )?;
            let index_buffer = Buffer::create(
                device,
                vk::BufferUsageFlags::INDEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                index_size * Self::MAX_INDEX_COUNT,
                &[],
            )?;
            geometry.push(Geometry {
                vertex_buffer,
                index_buffer,
            });
        }

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
            Shader::create(device, include_bytes!("../shaders/spv/imgui.vert"))?,
            Shader::create(device, include_bytes!("../shaders/spv/imgui.frag"))?,
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
            let vertex_binding_descriptions = [vk::VertexInputBindingDescription {
                binding: 0,
                stride: vertex_size as u32,
                input_rate: vk::VertexInputRate::VERTEX,
            }];
            let vertex_attribute_descriptions = [
                vk::VertexInputAttributeDescription {
                    location: 0,
                    binding: 0,
                    format: vk::Format::R32G32_SFLOAT,
                    offset: 0,
                },
                vk::VertexInputAttributeDescription {
                    location: 1,
                    binding: 0,
                    format: vk::Format::R32G32_SFLOAT,
                    offset: 2 * size_of::<f32>() as u32,
                },
                vk::VertexInputAttributeDescription {
                    location: 2,
                    binding: 0,
                    format: vk::Format::R8G8B8A8_UNORM,
                    offset: 4 * size_of::<f32>() as u32,
                },
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
                    .set_layouts(slice::from_ref(&desc_set_layout))
                    .push_constant_ranges(&[vk::PushConstantRange {
                        stage_flags: vk::ShaderStageFlags::VERTEX,
                        offset: 0,
                        size: size_of::<PushConstants>() as u32,
                    }]),
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

        // Font atlas.
        let font_atlas = {
            // Image.
            let image = device.create_image(
                &vk::ImageCreateInfo::builder()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: font_atlas_texture.width,
                        height: font_atlas_texture.height,
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
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    }),
                None,
            )?;

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

            // Staging buffer.
            let staging = Buffer::create(
                device,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                font_atlas_texture.data.len(),
                font_atlas_texture.data,
            )?;

            // Begin command buffer.
            let cmd = command_buffers[0];
            device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;

            // Transition image UNDEFINED -> TRANSFER_DST_OPTIMAL.
            device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                slice::from_ref(
                    &vk::ImageMemoryBarrier::builder()
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .image(image)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        }),
                ),
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
                    width: font_atlas_texture.width,
                    height: font_atlas_texture.height,
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
            device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                slice::from_ref(
                    &vk::ImageMemoryBarrier::builder()
                        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                        .dst_access_mask(vk::AccessFlags::SHADER_READ)
                        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .image(image)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        }),
                ),
            );

            // Submit.
            device.end_command_buffer(cmd)?;
            device.queue_submit(
                **device.queue(),
                slice::from_ref(&vk::SubmitInfo::builder().command_buffers(slice::from_ref(&cmd))),
                vk::Fence::null(),
            )?;
            device.queue_wait_idle(**device.queue())?;

            // Cleanup.
            device.free_command_buffers(command_pool, slice::from_ref(&cmd));
            device.destroy_command_pool(command_pool, None);
            staging.destroy(device);

            FontAtlas {
                handle: image,
                memory,
                view,
            }
        };

        Ok(Self {
            geometry,
            font_atlas,
            sampler,
            desc_set_layout,
            vertex_shader,
            fragment_shader,
            graphics_pipeline,
            pipeline_layout,
        })
    }

    pub unsafe fn update(
        &self,
        device: &Device,
        frame_index: u64,
        draw_data: &imgui::DrawData,
    ) -> Result<()> {
        let geometry = &self.geometry[frame_index as usize];
        let mut vertex_buffer = vec![];
        let mut index_buffer = vec![];
        for draw_list in draw_data.draw_lists() {
            vertex_buffer.extend_from_slice(draw_list.vtx_buffer());
            index_buffer.extend_from_slice(draw_list.idx_buffer());
        }
        geometry.vertex_buffer.copy_raw(device, &vertex_buffer)?;
        geometry.index_buffer.copy_raw(device, &index_buffer)?;
        Ok(())
    }

    pub unsafe fn draw(
        &self,
        device: &Device,
        cmd: vk::CommandBuffer,
        frame_index: u64,
        draw_data: &imgui::DrawData,
    ) {
        // Bind geometry buffers.
        let geometry = &self.geometry[frame_index as usize];
        device.cmd_bind_vertex_buffers(cmd, 0, slice::from_ref(&*geometry.vertex_buffer), &[0]);
        device.cmd_bind_index_buffer(cmd, *geometry.index_buffer, 0, vk::IndexType::UINT16);

        // Bind pipeline.
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.graphics_pipeline);

        // Update push constants.
        {
            let scale = na::vector![
                2.0 / draw_data.display_size[0],
                2.0 / draw_data.display_size[1]
            ];
            let translation = na::vector![
                -1.0 - draw_data.display_pos[0] * scale[0],
                -1.0 - draw_data.display_pos[1] * scale[1]
            ];
            device.cmd_push_constants(
                cmd,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                bytemuck::cast_slice(slice::from_ref(&PushConstants { scale, translation })),
            );
        }

        // Bind font atlas.
        {
            let image_info = *vk::DescriptorImageInfo::builder()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(self.font_atlas.view)
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
        }

        // Draw calls.
        let mut vertex_offset = 0;
        let mut index_offset = 0;
        for draw_list in draw_data.draw_lists() {
            for command in draw_list.commands() {
                match command {
                    imgui::DrawCmd::Elements {
                        count: index_count,
                        cmd_params,
                    } => {
                        let imgui::DrawCmdParams {
                            clip_rect,
                            vtx_offset,
                            idx_offset,
                            ..
                        } = cmd_params;

                        device.cmd_set_scissor(
                            cmd,
                            0,
                            &[vk::Rect2D {
                                offset: vk::Offset2D {
                                    x: clip_rect[0] as i32,
                                    y: clip_rect[1] as i32,
                                },
                                extent: vk::Extent2D {
                                    width: (clip_rect[2] - clip_rect[0]) as u32,
                                    height: (clip_rect[3] - clip_rect[1]) as u32,
                                },
                            }],
                        );

                        device.cmd_set_viewport(
                            cmd,
                            0,
                            slice::from_ref(&vk::Viewport {
                                x: 0.0,
                                y: 0.0,
                                width: draw_data.display_size[0],
                                height: draw_data.display_size[1],
                                min_depth: 0.0,
                                max_depth: 1.0,
                            }),
                        );

                        device.cmd_draw_indexed(
                            cmd,
                            index_count as u32,
                            1,
                            (idx_offset + index_offset) as u32,
                            (vtx_offset + vertex_offset) as i32,
                            0,
                        );
                    }
                    imgui::DrawCmd::ResetRenderState => todo!("ResetRenderState"),
                    imgui::DrawCmd::RawCallback { .. } => {
                        todo!("RawCallback")
                    }
                }
            }

            vertex_offset += draw_list.vtx_buffer().len();
            index_offset += draw_list.idx_buffer().len();
        }
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.destroy_image_view(self.font_atlas.view, None);
        device.destroy_image(self.font_atlas.handle, None);
        device.free_memory(self.font_atlas.memory, None);
        device.destroy_sampler(self.sampler, None);
        device.destroy_descriptor_set_layout(self.desc_set_layout, None);
        self.vertex_shader.destroy(device);
        self.fragment_shader.destroy(device);
        device.destroy_pipeline(self.graphics_pipeline, None);
        device.destroy_pipeline_layout(self.pipeline_layout, None);
        for geometry in &self.geometry {
            geometry.vertex_buffer.destroy(device);
            geometry.index_buffer.destroy(device);
        }
    }
}
