use super::*;

struct RasterMesh {
    positions: Buffer,
    normals: Buffer,
    indices: Buffer,
    index_count: u32,
    transform: na::Matrix4<f32>,
    base_color: LinSrgb,
}

impl RasterMesh {
    unsafe fn destroy(&self, device: &Device) {
        self.positions.destroy(device);
        self.normals.destroy(device);
        self.indices.destroy(device);
    }
}

#[repr(C)]
#[derive(Zeroable, Pod, Clone, Copy)]
struct PushConstants {
    transform: na::Matrix4<f32>,
    base_color: LinSrgba,
}

pub struct RasterScene {
    meshes: Vec<RasterMesh>,
    vertex_shader: Shader,
    fragment_shader: Shader,
    graphics_pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    clip_from_view: na::Matrix4<f32>,
    view_from_world: na::Matrix4<f32>,
}

impl RasterScene {
    pub unsafe fn create(device: &Device, assets_scene: &glb::Scene) -> Result<Self> {
        // Todo: Allocating meshes individually will eventually crash due to
        // `max_memory_allocation_count`, which is only 4096 on most NVIDIA
        // hardware. At that point, we need to start packing meshes into a
        // single allocation.
        let meshes = {
            let mut meshes = vec![];
            for assets_mesh in &assets_scene.meshes {
                let positions = assets_mesh.positions.0.as_ref();
                let normals = assets_mesh.normals.0.as_ref();
                let triangles = assets_mesh.triangles.0.as_ref();
                let transform = assets_mesh.transform;
                let base_color = assets_scene.materials[assets_mesh.material as usize].base_color;

                meshes.push(RasterMesh {
                    positions: Buffer::create_init(
                        device,
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        positions,
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
                    index_count: assets_mesh.index_count(),
                    transform,
                    base_color,
                });
            }
            meshes
        };

        // Pipelines.
        let (vertex_shader, fragment_shader) = (
            Shader::create(device, include_bytes!("../shaders/spv/triangle.vert"))?,
            Shader::create(device, include_bytes!("../shaders/spv/triangle.frag"))?,
        );
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
                .cull_mode(vk::CullModeFlags::BACK)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

            // Vertex input state.
            let position_binding_descriptions = vk::VertexInputBindingDescription::builder()
                .binding(0)
                .input_rate(vk::VertexInputRate::VERTEX)
                .stride((3 * size_of::<f32>()) as u32);
            let position_attribute_descriptions = vk::VertexInputAttributeDescription::builder()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(0);
            let normal_binding_descriptions = vk::VertexInputBindingDescription::builder()
                .binding(1)
                .input_rate(vk::VertexInputRate::VERTEX)
                .stride((3 * size_of::<f32>()) as u32);
            let normal_attribute_descriptions = vk::VertexInputAttributeDescription::builder()
                .binding(1)
                .location(1)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(0);
            let vertex_binding_descriptions =
                [*position_binding_descriptions, *normal_binding_descriptions];
            let vertex_attribute_descriptions = [
                *position_attribute_descriptions,
                *normal_attribute_descriptions,
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
            let pipeline_layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::builder().push_constant_ranges(&[
                        vk::PushConstantRange {
                            stage_flags: vk::ShaderStageFlags::VERTEX,
                            offset: 0,
                            size: size_of::<PushConstants>() as u32,
                        },
                    ]),
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

            (graphics_pipeline[0], pipeline_layout)
        };

        let (clip_from_view, view_from_world) = {
            let camera = &assets_scene.cameras[0];
            (
                *camera.clip_from_view().as_matrix(),
                camera.world_from_view().try_inverse().unwrap(),
            )
        };

        Ok(Self {
            meshes,
            vertex_shader,
            fragment_shader,
            graphics_pipeline,
            pipeline_layout,
            clip_from_view,
            view_from_world,
        })
    }

    pub unsafe fn draw(
        &self,
        device: &Device,
        cmd: vk::CommandBuffer,
        camera_transform: na::Matrix4<f32>,
    ) {
        // Prepare matrices.
        let clip_from_view = self.clip_from_view;
        let view_from_world = self.view_from_world;

        // Render meshes.
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.graphics_pipeline);
        for mesh in &self.meshes {
            // Prepare push constants.
            let push = PushConstants {
                // Pre-multiply all matrices to save space.
                // `max_push_constants_size` is typically in order of 128 to 256
                // bytes.
                transform: clip_from_view * view_from_world * camera_transform * mesh.transform,
                base_color: mesh.base_color.with_alpha(1.0),
            };
            let constants = bytemuck::cast_slice(slice::from_ref(&push));

            // Bind resources.
            device.cmd_bind_vertex_buffers(cmd, 0, slice::from_ref(&*mesh.positions), &[0]);
            device.cmd_bind_vertex_buffers(cmd, 1, slice::from_ref(&*mesh.normals), &[0]);
            device.cmd_bind_index_buffer(cmd, *mesh.indices, 0, vk::IndexType::UINT32);
            device.cmd_push_constants(
                cmd,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                constants,
            );

            // Draw.
            device.cmd_draw_indexed(cmd, mesh.index_count, 1, 0, 0, 0);
        }
    }

    pub unsafe fn destroy(&self, device: &Device) {
        self.vertex_shader.destroy(device);
        self.fragment_shader.destroy(device);
        for mesh in &self.meshes {
            mesh.destroy(device);
        }
        device.destroy_pipeline(self.graphics_pipeline, None);
        device.destroy_pipeline_layout(self.pipeline_layout, None);
    }
}
