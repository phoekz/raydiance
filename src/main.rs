#![deny(future_incompatible)]
#![deny(nonstandard_style)]
#![deny(clippy::pedantic)]
#![allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::wildcard_imports
)]

use std::{
    borrow::Cow,
    ffi::{CStr, CString},
    mem::{size_of, transmute},
    slice,
    time::Instant,
};

use anyhow::{anyhow, bail, ensure, Context, Result};
use ash::{
    extensions::khr::{Surface, Swapchain},
    vk::{self, SwapchainCreateInfoKHR},
};
use bytemuck::{Pod, Zeroable};
use nalgebra as na;
use palette::FromColor;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

#[macro_use]
extern crate log;

mod assets;

//
// Vulkan
//

const VULKAN_API_VERSION: u32 = vk::make_api_version(0, 1, 3, 0);
const MAX_CONCURRENT_FRAMES: u32 = 3;
const DEFAULT_SURFACE_FORMAT: vk::Format = vk::Format::B8G8R8A8_SRGB;
const DEFAULT_SURFACE_COLOR_SPACE: vk::ColorSpaceKHR = vk::ColorSpaceKHR::SRGB_NONLINEAR;
const DEFAULT_PRESENT_MODE: vk::PresentModeKHR = vk::PresentModeKHR::FIFO;
const DEFAULT_DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

struct VulkanDebug {
    utils: ash::extensions::ext::DebugUtils,
    callback: vk::DebugUtilsMessengerEXT,
}

struct VulkanSurface {
    handle: vk::SurfaceKHR,
    loader: ash::extensions::khr::Surface,
}

struct VulkanQueue {
    handle: vk::Queue,
    index: u32,
}

struct VulkanDevice {
    handle: ash::Device,
    physical_device: vk::PhysicalDevice,
    queue: VulkanQueue,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
}

impl VulkanDevice {
    unsafe fn find_memory_type_index(
        &self,
        property_flags: vk::MemoryPropertyFlags,
        memory_requirements: vk::MemoryRequirements,
    ) -> Result<u32> {
        let properties = &self.memory_properties;
        let requirements = &memory_requirements;
        properties.memory_types[..properties.memory_type_count as _]
            .iter()
            .enumerate()
            .find(|&(index, memory_type)| {
                (1 << index) & requirements.memory_type_bits != 0
                    && memory_type.property_flags & property_flags == property_flags
            })
            .map(|(index, _)| index as u32)
            .ok_or_else(|| {
                anyhow!(
                    "Unable to find suitable memory type for the buffer, requirements={memory_requirements:?}"
                )
            })
    }
}

struct VulkanSwapchain {
    handle: vk::SwapchainKHR,
    loader: ash::extensions::khr::Swapchain,
    images: Vec<(vk::Image, vk::ImageView)>,
}

impl VulkanSwapchain {
    const PRE_TRANSFORM: vk::SurfaceTransformFlagsKHR = vk::SurfaceTransformFlagsKHR::IDENTITY;
    const COMPOSITE_TRANSFORM: vk::CompositeAlphaFlagsKHR = vk::CompositeAlphaFlagsKHR::OPAQUE;
    const IMAGE_USAGE: vk::ImageUsageFlags = vk::ImageUsageFlags::COLOR_ATTACHMENT;

    unsafe fn create(
        instance: &ash::Instance,
        surface: &VulkanSurface,
        device: &VulkanDevice,
        window_size: vk::Extent2D,
    ) -> Result<Self> {
        // Validate surface format.
        {
            let surface_formats = surface
                .loader
                .get_physical_device_surface_formats(device.physical_device, surface.handle)
                .context("Getting surface formats")?;
            if !surface_formats.contains(&vk::SurfaceFormatKHR {
                format: DEFAULT_SURFACE_FORMAT,
                color_space: DEFAULT_SURFACE_COLOR_SPACE,
            }) {
                bail!("Surface must support format={DEFAULT_SURFACE_FORMAT:?} and color_space={DEFAULT_SURFACE_COLOR_SPACE:?}");
            }
        }

        // Validate surface capabilities.
        {
            let surface_capabilities = surface
                .loader
                .get_physical_device_surface_capabilities(device.physical_device, surface.handle)
                .context("Getting surface capabilities")?;

            if MAX_CONCURRENT_FRAMES < surface_capabilities.min_image_count {
                bail!(
                    "MAX_CONCURRENT_FRAMES={} must be >= min_image_count={}",
                    MAX_CONCURRENT_FRAMES,
                    surface_capabilities.min_image_count
                );
            }

            if MAX_CONCURRENT_FRAMES > surface_capabilities.max_image_count {
                bail!(
                    "MAX_CONCURRENT_FRAMES={} must be <= max_image_count={}",
                    MAX_CONCURRENT_FRAMES,
                    surface_capabilities.max_image_count
                );
            }
            if !surface_capabilities
                .supported_transforms
                .contains(Self::PRE_TRANSFORM)
            {
                bail!("Surface must support {:?}", Self::PRE_TRANSFORM);
            }

            if !surface_capabilities
                .supported_composite_alpha
                .contains(Self::COMPOSITE_TRANSFORM)
            {
                bail!("Surface must support {:?}", Self::COMPOSITE_TRANSFORM);
            }

            if !surface_capabilities
                .supported_usage_flags
                .contains(Self::IMAGE_USAGE)
            {
                bail!("Surface must support {:?}", Self::IMAGE_USAGE);
            }
        }

        // Validate surface present mode.
        {
            let surface_present_mode = surface
                .loader
                .get_physical_device_surface_present_modes(device.physical_device, surface.handle)
                .context("Getting surface present modes")?;
            if !surface_present_mode.contains(&DEFAULT_PRESENT_MODE) {
                bail!("Surface must support {DEFAULT_PRESENT_MODE:?}");
            }
        }

        // Create loader.
        let loader = Swapchain::new(instance, &device.handle);

        // Create swapchain.
        let mut swapchain = Self {
            handle: vk::SwapchainKHR::null(),
            loader,
            images: vec![],
        };
        swapchain
            .recreate(surface, device, window_size)
            .context("Creating swapchain")?;

        Ok(swapchain)
    }

    unsafe fn destroy(&mut self, device: &VulkanDevice) {
        self.images.iter().for_each(|&images| {
            device.handle.destroy_image_view(images.1, None);
        });
        self.images.clear();
        if self.handle != vk::SwapchainKHR::null() {
            self.loader.destroy_swapchain(self.handle, None);
        }
    }

    unsafe fn recreate(
        &mut self,
        surface: &VulkanSurface,
        device: &VulkanDevice,
        window_size: vk::Extent2D,
    ) -> Result<()> {
        // Flush pipeline.
        device
            .handle
            .device_wait_idle()
            .context("Waiting for device idle")?;

        // Destroy old swapchain.
        self.destroy(device);

        // Create new swapchain.
        let swapchain = self
            .loader
            .create_swapchain(
                &SwapchainCreateInfoKHR::builder()
                    .surface(surface.handle)
                    .min_image_count(MAX_CONCURRENT_FRAMES)
                    .image_format(DEFAULT_SURFACE_FORMAT)
                    .image_color_space(DEFAULT_SURFACE_COLOR_SPACE)
                    .image_extent(window_size)
                    .image_array_layers(1)
                    .image_usage(Self::IMAGE_USAGE)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .pre_transform(Self::PRE_TRANSFORM)
                    .composite_alpha(Self::COMPOSITE_TRANSFORM)
                    .present_mode(DEFAULT_PRESENT_MODE)
                    .clipped(true),
                None,
            )
            .context("Creating swapchain")?;
        let images = self
            .loader
            .get_swapchain_images(swapchain)
            .context("Getting swapchain images")?;
        let image_views = images
            .iter()
            .map(|&image| {
                let create_view_info = vk::ImageViewCreateInfo::builder()
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(DEFAULT_SURFACE_FORMAT)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::IDENTITY,
                        g: vk::ComponentSwizzle::IDENTITY,
                        b: vk::ComponentSwizzle::IDENTITY,
                        a: vk::ComponentSwizzle::IDENTITY,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image(image);
                device
                    .handle
                    .create_image_view(&create_view_info, None)
                    .context("Creating image view")
            })
            .collect::<Result<Vec<_>>>()
            .context("Creating swapchain image views")?;
        let images = images.into_iter().zip(image_views.into_iter()).collect();

        self.handle = swapchain;
        self.images = images;
        Ok(())
    }
}

struct VulkanShader {
    module: vk::ShaderModule,
}

impl VulkanShader {
    unsafe fn create(device: &VulkanDevice, bytes: &[u8]) -> Result<Self> {
        let dwords = bytes
            .chunks_exact(4)
            .map(|chunk| transmute([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect::<Vec<u32>>();
        let module = device
            .handle
            .create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(&dwords), None)
            .context("Compiling shader")?;
        Ok(Self { module })
    }

    unsafe fn destroy(&self, device: &VulkanDevice) {
        device.handle.destroy_shader_module(self.module, None);
    }
}

struct VulkanBuffer {
    handle: vk::Buffer,
    memory: vk::DeviceMemory,
    byte_count: usize,
}

impl VulkanBuffer {
    unsafe fn create(
        device: &VulkanDevice,
        usage_flags: vk::BufferUsageFlags,
        property_flags: vk::MemoryPropertyFlags,
        byte_count: usize,
        bytes: &[u8],
    ) -> Result<Self> {
        // Create initial buffer.
        let buffer = device
            .handle
            .create_buffer(
                &vk::BufferCreateInfo::builder()
                    .size(byte_count as u64)
                    .usage(usage_flags)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
            .with_context(|| format!("Creating buffer of bytes={byte_count}"))?;

        // Find memory type index.
        let requirements = device.handle.get_buffer_memory_requirements(buffer);
        let index = device.find_memory_type_index(property_flags, requirements)?;

        // Create allocation.
        let allocate_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(requirements.size)
            .memory_type_index(index)
            .build();
        let memory = device.handle.allocate_memory(&allocate_info, None)?;

        // Copy to staging buffer.
        if !bytes.is_empty() {
            let map_flags = vk::MemoryMapFlags::empty();
            let ptr = device
                .handle
                .map_memory(memory, 0, requirements.size, map_flags)?;
            let ptr = ptr.cast::<u8>();
            let mapped_slice = std::slice::from_raw_parts_mut(ptr, byte_count);
            mapped_slice.copy_from_slice(bytes);
            device.handle.unmap_memory(memory);
        }

        // Bind memory.
        device.handle.bind_buffer_memory(buffer, memory, 0)?;

        Ok(Self {
            handle: buffer,
            memory,
            byte_count,
        })
    }

    unsafe fn create_init<T: Copy + Zeroable + Pod>(
        device: &VulkanDevice,
        usage_flags: vk::BufferUsageFlags,
        elements: &[T],
    ) -> Result<Self> {
        // Calculate sizes.
        let element_byte_count = size_of::<T>();
        let byte_count = element_byte_count * elements.len();

        // Create host buffer.
        let host_usage_flags = vk::BufferUsageFlags::TRANSFER_SRC;
        let property_flags = vk::MemoryPropertyFlags::HOST_VISIBLE;
        let bytes = bytemuck::cast_slice(elements);
        let host_buffer =
            Self::create(device, host_usage_flags, property_flags, byte_count, bytes)?;

        // Create device buffer.
        let device_usage_flags =
            usage_flags | vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::TRANSFER_DST;
        let property_flags = vk::MemoryPropertyFlags::DEVICE_LOCAL;
        let device_buffer =
            Self::create(device, device_usage_flags, property_flags, byte_count, &[])?;

        // Copy.
        host_buffer.copy_to(device, &device_buffer)?;

        // Cleanup.
        host_buffer.destroy(device);

        Ok(device_buffer)
    }

    unsafe fn copy_to(&self, device: &VulkanDevice, dst: &Self) -> Result<()> {
        // Validate.
        if self.byte_count != dst.byte_count {
            bail!(
                "src and dst must have the same size, got src={} and dst={} instead",
                self.byte_count,
                dst.byte_count
            );
        }

        // Create temporary upload setup.
        let pool = device.handle.create_command_pool(
            &vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(device.queue.index),
            None,
        )?;
        let cmd = device.handle.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::builder()
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1),
        )?[0];
        let fence = device.handle.create_fence(
            &vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::empty()),
            None,
        )?;

        // Record commands.
        device
            .handle
            .begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::builder())?;
        device.handle.cmd_copy_buffer(
            cmd,
            self.handle,
            dst.handle,
            &[vk::BufferCopy {
                src_offset: 0,
                dst_offset: 0,
                size: self.byte_count as u64,
            }],
        );
        device.handle.end_command_buffer(cmd)?;

        // Submit and wait.
        device.handle.queue_submit(
            device.queue.handle,
            &[*vk::SubmitInfo::builder().command_buffers(&[cmd])],
            fence,
        )?;
        device.handle.wait_for_fences(&[fence], true, u64::MAX)?;

        // Cleanup.
        device.handle.destroy_fence(fence, None);
        device.handle.free_command_buffers(pool, &[cmd]);
        device.handle.destroy_command_pool(pool, None);

        Ok(())
    }

    unsafe fn destroy(&self, device: &VulkanDevice) {
        device.handle.destroy_buffer(self.handle, None);
        device.handle.free_memory(self.memory, None);
    }
}

struct VulkanMesh {
    positions: VulkanBuffer,
    normals: VulkanBuffer,
    indices: VulkanBuffer,
    index_count: u32,
    transform: na::Matrix4<f32>,
    base_color: na::Vector4<f32>,
}

impl VulkanMesh {
    unsafe fn destroy(&self, device: &VulkanDevice) {
        self.positions.destroy(device);
        self.normals.destroy(device);
        self.indices.destroy(device);
    }
}

struct VulkanDepth {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
}

impl VulkanDepth {
    unsafe fn create(device: &VulkanDevice, window_size: vk::Extent2D) -> Result<Self> {
        // Image.
        let image = device.handle.create_image(
            &vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(DEFAULT_DEPTH_FORMAT)
                .extent(vk::Extent3D {
                    width: window_size.width,
                    height: window_size.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT),
            None,
        )?;

        // Allocate memory.
        let requirements = device.handle.get_image_memory_requirements(image);
        let index =
            device.find_memory_type_index(vk::MemoryPropertyFlags::DEVICE_LOCAL, requirements)?;
        let memory = device.handle.allocate_memory(
            &vk::MemoryAllocateInfo::builder()
                .allocation_size(requirements.size)
                .memory_type_index(index),
            None,
        )?;
        device.handle.bind_image_memory(image, memory, 0)?;

        // Image view.
        let view = device.handle.create_image_view(
            &vk::ImageViewCreateInfo::builder()
                .view_type(vk::ImageViewType::TYPE_2D)
                .image(image)
                .format(DEFAULT_DEPTH_FORMAT)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::DEPTH,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                }),
            None,
        )?;

        Ok(Self {
            image,
            memory,
            view,
        })
    }

    unsafe fn recreate(&mut self, device: &VulkanDevice, window_size: vk::Extent2D) -> Result<()> {
        self.destroy(device);
        *self = Self::create(device, window_size)?;
        Ok(())
    }

    unsafe fn destroy(&self, device: &VulkanDevice) {
        let device = &device.handle;
        device.destroy_image_view(self.view, None);
        device.destroy_image(self.image, None);
        device.free_memory(self.memory, None);
    }
}

#[repr(C)]
#[derive(Zeroable, Pod, Clone, Copy)]
struct VulkanPushConstants {
    transform: na::Matrix4<f32>,
    base_color: na::Vector4<f32>,
}

struct VulkanScene {
    meshes: Vec<VulkanMesh>,
    vertex_shader: VulkanShader,
    fragment_shader: VulkanShader,
    graphics_pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
}

impl VulkanScene {
    unsafe fn create(device: &VulkanDevice, assets_scene: &assets::Scene) -> Result<Self> {
        // Todo: Allocating meshes individually will eventually crash due to
        // `max_memory_allocation_count`, which is only 4096 on most NVIDIA
        // hardware. At that point, we need to start packing meshes into a
        // single allocation.
        let meshes = {
            let mut meshes = vec![];
            for assets_mesh in &assets_scene.meshes {
                let positions = assets_mesh.positions.0.as_ref();
                let normals = assets_mesh.normals.0.as_ref();
                let indices = assets_mesh.indices.0.as_ref();
                let transform = assets_mesh.transform;
                let base_color = assets_mesh.material.base_color;

                meshes.push(VulkanMesh {
                    positions: VulkanBuffer::create_init(
                        device,
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        positions,
                    )?,
                    normals: VulkanBuffer::create_init(
                        device,
                        vk::BufferUsageFlags::VERTEX_BUFFER,
                        normals,
                    )?,
                    indices: VulkanBuffer::create_init(
                        device,
                        vk::BufferUsageFlags::INDEX_BUFFER,
                        indices,
                    )?,
                    index_count: assets_mesh.indices.index_count(),
                    transform,
                    base_color,
                });
            }
            meshes
        };

        // Pipelines.
        let (vertex_shader, fragment_shader) = (
            VulkanShader::create(device, include_bytes!("shaders/spv/triangle.vert"))?,
            VulkanShader::create(device, include_bytes!("shaders/spv/triangle.frag"))?,
        );
        let (graphics_pipeline, pipeline_layout) = {
            // Stages.
            let entry_point = CStr::from_bytes_with_nul(b"main\0")?;
            let vertex_stage = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vertex_shader.module)
                .name(entry_point);
            let fragment_stage = vk::PipelineShaderStageCreateInfo::builder()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fragment_shader.module)
                .name(entry_point);
            let stages = [*vertex_stage, *fragment_stage];

            // Rasterizer.
            let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
                .polygon_mode(vk::PolygonMode::FILL)
                .line_width(1.0)
                .cull_mode(vk::CullModeFlags::BACK)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

            // Vertex input.
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

            // Input assembly.
            let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

            // Dynamic state.
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
                .dynamic_states(&[vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR]);

            // Viewport stage.
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

            // Rendering.
            let mut rendering = vk::PipelineRenderingCreateInfo::builder()
                .color_attachment_formats(&[DEFAULT_SURFACE_FORMAT])
                .depth_attachment_format(DEFAULT_DEPTH_FORMAT);

            // Pipeline layout.
            let pipeline_layout = device
                .handle
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::builder().push_constant_ranges(&[
                        vk::PushConstantRange {
                            stage_flags: vk::ShaderStageFlags::VERTEX,
                            offset: 0,
                            size: size_of::<VulkanPushConstants>() as u32,
                        },
                    ]),
                    None,
                )
                .context("Creating pipeline layout")?;

            // Pipeline.
            let graphics_pipeline = device
                .handle
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
                            .push_next(&mut rendering)
                            .layout(pipeline_layout),
                    ),
                    None,
                )
                .unwrap();

            (graphics_pipeline[0], pipeline_layout)
        };

        Ok(Self {
            meshes,
            vertex_shader,
            fragment_shader,
            graphics_pipeline,
            pipeline_layout,
        })
    }

    unsafe fn draw(
        &self,
        device: &VulkanDevice,
        cmd: vk::CommandBuffer,
        time: f32,
        assets_scene: &assets::Scene,
    ) {
        // Prepare matrices.
        let camera = &assets_scene.cameras[0];
        let projection = *camera.projection().as_matrix();
        let view = camera.view().try_inverse().unwrap();
        let rotation = na::Matrix4::from_axis_angle(&na::Vector3::y_axis(), time);

        // Render meshes.
        let device = &device.handle;
        device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.graphics_pipeline);
        for mesh in &self.meshes {
            // Prepare push constants.
            let push = VulkanPushConstants {
                // Pre-multiply all matrices to save space.
                // `max_push_constants_size` is typically in order of 128 to 256
                // bytes.
                transform: projection * view * rotation * mesh.transform,
                base_color: mesh.base_color,
            };
            let constants = bytemuck::cast_slice(slice::from_ref(&push));

            // Bind resources.
            device.cmd_bind_vertex_buffers(cmd, 0, slice::from_ref(&mesh.positions.handle), &[0]);
            device.cmd_bind_vertex_buffers(cmd, 1, slice::from_ref(&mesh.normals.handle), &[0]);
            device.cmd_bind_index_buffer(cmd, mesh.indices.handle, 0, vk::IndexType::UINT32);
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

    unsafe fn destroy(&self, device: &VulkanDevice) {
        self.vertex_shader.destroy(device);
        self.fragment_shader.destroy(device);
        for mesh in &self.meshes {
            mesh.destroy(device);
        }

        let device = &device.handle;
        device.destroy_pipeline(self.graphics_pipeline, None);
        device.destroy_pipeline_layout(self.pipeline_layout, None);
    }
}

struct VulkanContext {
    instance: ash::Instance,
    debug: Option<VulkanDebug>,
    surface: VulkanSurface,
    device: VulkanDevice,
    swapchain: VulkanSwapchain,
    depth: VulkanDepth,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    present_complete: Vec<vk::Semaphore>,
    rendering_complete: Vec<vk::Semaphore>,
    draw_commands_reuse: Vec<vk::Fence>,

    scene: VulkanScene,
}

//
// Window
//

#[derive(Clone, Copy, PartialEq, Eq)]
struct WindowSize {
    w: u32,
    h: u32,
}

impl WindowSize {
    fn is_zero(self) -> bool {
        self.w == 0 && self.h == 0
    }
}

impl From<WindowSize> for PhysicalSize<u32> {
    fn from(value: WindowSize) -> Self {
        Self {
            width: value.w,
            height: value.h,
        }
    }
}

impl From<PhysicalSize<u32>> for WindowSize {
    fn from(value: PhysicalSize<u32>) -> Self {
        Self {
            w: value.width,
            h: value.height,
        }
    }
}

impl From<WindowSize> for vk::Extent2D {
    fn from(value: WindowSize) -> Self {
        Self {
            width: value.w,
            height: value.h,
        }
    }
}

//
// Main
//

fn main() -> Result<()> {
    // Init logging.
    env_logger::init();

    // Init winit.
    let window_title = env!("CARGO_PKG_NAME");
    let min_window_size = WindowSize { w: 320, h: 180 };
    let mut window_size = WindowSize {
        w: 1280 / 4,
        h: 720 / 4,
    };
    let mut resized_window_size = window_size;
    let (mut event_loop, window) = {
        // Create event loop.
        let event_loop = EventLoop::new();

        // Build window.
        let window = WindowBuilder::new()
            .with_title(window_title)
            .with_inner_size::<PhysicalSize<_>>(window_size.into())
            .with_min_inner_size::<PhysicalSize<_>>(min_window_size.into())
            .with_always_on_top(true)
            .with_resizable(true)
            .build(&event_loop)
            .context("Building winit window")?;

        // Get primary monitor dimensions.
        let (monitor_width, monitor_height) = {
            let monitor = window
                .primary_monitor()
                .context("Getting primary monitor")?;
            (monitor.size().width, monitor.size().height)
        };
        info!("Primary monitor dimensions: {monitor_width} x {monitor_height}");

        // Center window.
        window.set_outer_position(PhysicalPosition::new(
            (monitor_width - window_size.w) / 2,
            (monitor_height - window_size.h) / 2,
        ));

        (event_loop, window)
    };

    // Init scene.
    let assets_scene = assets::Scene::create(include_bytes!("assets/rounded_cube.glb"))?;

    // Init Vulkan.
    let vulkan_validation = std::env::var("VULKAN_VALIDATION").is_ok();
    if vulkan_validation {
        info!("Vulkan validation layers enabled");
    }
    let vulkan_entry = unsafe { ash::Entry::load().context("Loading ash entry")? };
    let mut vulkan = unsafe {
        let instance = {
            // Application metadata.
            let application_name = CString::new(window_title)?;
            let engine_name = CString::new(window_title)?;
            let application_info = vk::ApplicationInfo::builder()
                .application_name(application_name.as_c_str())
                .application_version(1)
                .engine_name(engine_name.as_c_str())
                .engine_version(1)
                .api_version(VULKAN_API_VERSION);

            // Layers.
            let enabled_layers = {
                let check_support =
                    |layers: &[vk::LayerProperties], layer_name: &CStr| -> Result<()> {
                        if layers
                            .iter()
                            .any(|layer| CStr::from_ptr(layer.layer_name.as_ptr()) == layer_name)
                        {
                            return Ok(());
                        }
                        bail!("Instance must support layer={layer_name:?}");
                    };

                let layers = vulkan_entry
                    .enumerate_instance_layer_properties()
                    .context("Getting instance layers")?;
                debug!("{layers:#?}");

                let khronos_validation =
                    CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0")?;
                check_support(&layers, khronos_validation)?;
                let mut enabled_layers = vec![];
                if vulkan_validation {
                    enabled_layers.push(khronos_validation.as_ptr());
                }
                enabled_layers
            };

            // Extensions.
            let enabled_extensions = {
                let check_support =
                    |extensions: &[vk::ExtensionProperties], extension_name: &CStr| -> Result<()> {
                        if extensions.iter().any(|extension| {
                            CStr::from_ptr(extension.extension_name.as_ptr()) == extension_name
                        }) {
                            return Ok(());
                        }
                        bail!("Instance must support extension={:?}", extension_name);
                    };

                let extensions = vulkan_entry
                    .enumerate_instance_extension_properties(None)
                    .context("Getting instance extensions")?;
                debug!("{extensions:#?}");

                let mut enabled_extensions = vec![];
                enabled_extensions.push(vk::KhrSurfaceFn::name());
                enabled_extensions.push(vk::KhrWin32SurfaceFn::name());
                if vulkan_validation {
                    enabled_extensions.push(vk::ExtDebugUtilsFn::name());
                }
                for enabled_extension in &enabled_extensions {
                    check_support(&extensions, enabled_extension)?;
                }
                enabled_extensions
                    .into_iter()
                    .map(CStr::as_ptr)
                    .collect::<Vec<_>>()
            };

            // Create.
            vulkan_entry
                .create_instance(
                    &vk::InstanceCreateInfo::builder()
                        .application_info(&application_info)
                        .enabled_layer_names(&enabled_layers)
                        .enabled_extension_names(&enabled_extensions),
                    None,
                )
                .context("Creating Vulkan instance")?
        };

        let debug = if vulkan_validation {
            unsafe extern "system" fn debug_callback(
                message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
                message_type: vk::DebugUtilsMessageTypeFlagsEXT,
                p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
                _user_data: *mut std::os::raw::c_void,
            ) -> vk::Bool32 {
                let callback_data = *p_callback_data;
                let message_id_number = callback_data.message_id_number;
                let message_id_name = if callback_data.p_message_id_name.is_null() {
                    Cow::from("")
                } else {
                    CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
                };
                let message = if callback_data.p_message.is_null() {
                    Cow::from("")
                } else {
                    CStr::from_ptr(callback_data.p_message).to_string_lossy()
                };

                #[allow(clippy::match_same_arms)]
                let level = match message_severity {
                    vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => log::Level::Debug,
                    vk::DebugUtilsMessageSeverityFlagsEXT::INFO => log::Level::Info,
                    vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => log::Level::Warn,
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => log::Level::Error,
                    _ => log::Level::Warn,
                };

                log!(level, "Vulkan: type={message_type:?} id={message_id_number:?} name={message_id_name:?} message={message}");

                vk::FALSE
            }

            let utils = ash::extensions::ext::DebugUtils::new(&vulkan_entry, &instance);
            let callback = utils
                .create_debug_utils_messenger(
                    &vk::DebugUtilsMessengerCreateInfoEXT::builder()
                        .message_severity(
                            vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
                        )
                        .message_type(
                            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                        )
                        .pfn_user_callback(Some(debug_callback)),
                    None,
                )
                .context("Creating Vulkan debug utils messenger")?;

            Some(VulkanDebug { utils, callback })
        } else {
            None
        };

        let surface = {
            let surface = ash_window::create_surface(
                &vulkan_entry,
                &instance,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None,
            )
            .context("Creating surface")?;
            let loader = Surface::new(&vulkan_entry, &instance);

            VulkanSurface {
                handle: surface,
                loader,
            }
        };

        let device = {
            // Find physical device and its queues.
            let (physical_device, queue_family_index) =
                if let Some((physical_device, queue_families)) = instance
                    .enumerate_physical_devices()
                    .context("Enumerating physical devices")?
                    .into_iter()
                    .find_map(|physical_device| {
                        // Make sure the device supports the features we need.
                        let mut features_11 = vk::PhysicalDeviceVulkan11Features::default();
                        let mut features_12 = vk::PhysicalDeviceVulkan12Features::default();
                        let mut features_13 = vk::PhysicalDeviceVulkan13Features::default();
                        let mut features = vk::PhysicalDeviceFeatures2::builder()
                            .push_next(&mut features_11)
                            .push_next(&mut features_12)
                            .push_next(&mut features_13);
                        instance.get_physical_device_features2(physical_device, &mut features);
                        let features_10 = features.features;
                        debug!("{:#?}", features_10);
                        debug!("{:#?}", features_11);
                        debug!("{:#?}", features_12);
                        debug!("{:#?}", features_13);
                        if features_13.dynamic_rendering == vk::FALSE {
                            return None;
                        }

                        // We only support discrete GPUs at this point.
                        let properties = instance.get_physical_device_properties(physical_device);
                        if properties.device_type != vk::PhysicalDeviceType::DISCRETE_GPU {
                            return None;
                        }
                        let limits = properties.limits;
                        info!(
                            "max_push_constants_size: {}",
                            limits.max_push_constants_size
                        );
                        info!(
                            "max_memory_allocation_count: {}",
                            limits.max_memory_allocation_count
                        );

                        // Make sure the device supports the queue types we
                        // need. Todo: We assume that there is at least one
                        // queue that supports all operations. This might not be
                        // true on all devices, so we need to come back later to
                        // generalize this.
                        let queue_families =
                            instance.get_physical_device_queue_family_properties(physical_device);
                        let queue = queue_families.into_iter().enumerate().find_map(
                            |(queue_family_index, queue_family)| {
                                if queue_family.queue_flags.contains(
                                    vk::QueueFlags::GRAPHICS
                                        | vk::QueueFlags::COMPUTE
                                        | vk::QueueFlags::TRANSFER,
                                ) {
                                    return Some(queue_family_index);
                                }
                                None
                            },
                        );
                        let queue = if let Some(queue) = queue {
                            queue as u32
                        } else {
                            return None;
                        };

                        // Check for present support.
                        if let Ok(supports_present) =
                            surface.loader.get_physical_device_surface_support(
                                physical_device,
                                queue,
                                surface.handle,
                            )
                        {
                            if !supports_present {
                                return None;
                            }
                        } else {
                            return None;
                        }

                        Some((physical_device, queue))
                    })
                {
                    (physical_device, queue_families)
                } else {
                    bail!("Failed to find any suitable physical devices");
                };
            let device_properties = instance.get_physical_device_properties(physical_device);
            info!(
                "Physical device: {:?}",
                CStr::from_ptr(device_properties.device_name.as_ptr())
            );

            // Create device.
            let device = {
                // Queue infos.
                let queue_create_info = vk::DeviceQueueCreateInfo::builder()
                    .queue_family_index(queue_family_index)
                    .queue_priorities(&[1.0])
                    .build();
                let queue_create_infos = [queue_create_info];

                // Extensions.
                let enabled_extensions = {
                    let check_support = |extensions: &[vk::ExtensionProperties],
                                         extension_name: &CStr|
                     -> Result<()> {
                        if extensions.iter().any(|extension| {
                            CStr::from_ptr(extension.extension_name.as_ptr()) == extension_name
                        }) {
                            return Ok(());
                        }
                        bail!("Device must support extension={extension_name:?}");
                    };

                    let extensions = instance
                        .enumerate_device_extension_properties(physical_device)
                        .context("Getting device extensions")?;
                    debug!("{extensions:#?}");

                    let enabled_extensions = [vk::KhrSwapchainFn::name()];
                    for extension in &enabled_extensions {
                        check_support(&extensions, extension)?;
                    }
                    enabled_extensions
                        .into_iter()
                        .map(CStr::as_ptr)
                        .collect::<Vec<_>>()
                };

                // Features.
                let mut dynamic_rendering_feature =
                    vk::PhysicalDeviceDynamicRenderingFeatures::builder().dynamic_rendering(true);

                // Create.
                instance
                    .create_device(
                        physical_device,
                        &vk::DeviceCreateInfo::builder()
                            .queue_create_infos(&queue_create_infos)
                            .enabled_extension_names(&enabled_extensions)
                            .push_next(&mut dynamic_rendering_feature),
                        None,
                    )
                    .context("Creating device")?
            };

            // Memory properties.
            let memory_properties = instance.get_physical_device_memory_properties(physical_device);

            // Create queue.
            let queue = VulkanQueue {
                index: queue_family_index,
                handle: device.get_device_queue(queue_family_index, 0),
            };

            VulkanDevice {
                handle: device,
                physical_device,
                queue,
                memory_properties,
            }
        };

        let command_pool = {
            device
                .handle
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::builder()
                        .queue_family_index(device.queue.index)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .context("Creating command pool")?
        };

        let command_buffers = {
            device
                .handle
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::builder()
                        .command_buffer_count(MAX_CONCURRENT_FRAMES)
                        .command_pool(command_pool)
                        .level(vk::CommandBufferLevel::PRIMARY),
                )
                .context("Allocating command buffers")?
        };

        let swapchain = VulkanSwapchain::create(&instance, &surface, &device, window_size.into())
            .context("Creating swapchain")?;

        let depth = VulkanDepth::create(&device, window_size.into()).context("Creating depth")?;

        let (present_complete, rendering_complete, draw_commands_reuse) = {
            let mut present_complete = vec![];
            let mut rendering_complete = vec![];
            let mut draw_commands_reuse = vec![];
            for _ in 0..MAX_CONCURRENT_FRAMES {
                present_complete.push(
                    device
                        .handle
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                        .context("Creating semaphore")?,
                );

                rendering_complete.push(
                    device
                        .handle
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                        .context("Creating semaphore")?,
                );

                draw_commands_reuse.push(
                    device
                        .handle
                        .create_fence(
                            &vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED),
                            None,
                        )
                        .context("Creating fence")?,
                );
            }
            (present_complete, rendering_complete, draw_commands_reuse)
        };

        let scene = VulkanScene::create(&device, &assets_scene)?;

        VulkanContext {
            instance,
            debug,
            surface,
            device,
            swapchain,
            depth,
            command_pool,
            command_buffers,
            present_complete,
            rendering_complete,
            draw_commands_reuse,

            scene,
        }
    };

    // Main event loop.
    let instant_start = Instant::now();
    let mut frame_index = 0_u64;
    let mut frame_count = 0_u64;
    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        // Event handler.
        match event {
            // Close window if user hits the X.
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,

            // Close window if user hits the escape key.
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    },
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,

            Event::WindowEvent {
                event: WindowEvent::Resized(new_window_size),
                window_id,
            } if window_id == window.id() => {
                // Ignore the incorrect resized events at frame_count=0. Issue:
                // https://github.com/rust-windowing/winit/issues/2094
                debug!(
                    "New window size: {} x {}",
                    new_window_size.width, new_window_size.height
                );

                if frame_count == 0 {
                    debug!("Ignore resized event at frame_count={frame_count}");
                } else {
                    resized_window_size = new_window_size.into();
                }
            }

            Event::MainEventsCleared => {
                window.request_redraw();
            }

            Event::RedrawRequested(_) => {
                // Todo: Decouple swapchain from rendering and handle swapchain
                // resizing and minimizing logic separately from the application
                // logic.
                unsafe {
                    let device = &vulkan.device.handle;
                    let queue = &vulkan.device.queue.handle;

                    // Wait until previous frame is done.
                    device
                        .wait_for_fences(
                            slice::from_ref(&vulkan.draw_commands_reuse[frame_index as usize]),
                            true,
                            u64::MAX,
                        )
                        .context("Waiting for fence")
                        .unwrap();

                    // Stop rendering if is minimized (size equals to zero).
                    if resized_window_size.is_zero() {
                        return;
                    }

                    // Acquire image.
                    let acquire_result = vulkan
                        .swapchain
                        .loader
                        .acquire_next_image(
                            vulkan.swapchain.handle,
                            u64::MAX,
                            vulkan.present_complete[frame_index as usize],
                            vk::Fence::null(),
                        )
                        .context("Acquiring next image");
                    let present_index = if let Ok((present_index, _)) = acquire_result {
                        present_index
                    } else {
                        vulkan
                            .swapchain
                            .recreate(&vulkan.surface, &vulkan.device, window_size.into())
                            .context("Recreating swapchain")
                            .unwrap();
                        vulkan
                            .depth
                            .recreate(&vulkan.device, window_size.into())
                            .context("Recreating depth")
                            .unwrap();
                        return;
                    };

                    // Synchronize previous frame.
                    device
                        .reset_fences(slice::from_ref(
                            &vulkan.draw_commands_reuse[frame_index as usize],
                        ))
                        .context("Resetting fences")
                        .unwrap();

                    // Setup dynamic rendering.
                    let hue = (frame_count % 2000) as f32 / 2000.0;
                    let hsv = palette::Hsv::with_wp(hue * 360.0, 0.75, 1.0);
                    let rgb = palette::LinSrgb::from_color(hsv);
                    let color_attachment = vk::RenderingAttachmentInfo::builder()
                        .image_view(vulkan.swapchain.images[present_index as usize].1)
                        .image_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
                        .load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(vk::ClearValue {
                            color: vk::ClearColorValue {
                                float32: [rgb.red, rgb.green, rgb.blue, 1.0],
                            },
                        });
                    let depth_attachment = vk::RenderingAttachmentInfo::builder()
                        .image_view(vulkan.depth.view)
                        .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                        .load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(vk::ClearValue {
                            depth_stencil: vk::ClearDepthStencilValue {
                                depth: 1.0,
                                stencil: 0,
                            },
                        });

                    let rendering_info = vk::RenderingInfo::builder()
                        .render_area(vk::Rect2D {
                            offset: vk::Offset2D::default(),
                            extent: window_size.into(),
                        })
                        .layer_count(1)
                        .color_attachments(slice::from_ref(&color_attachment))
                        .depth_attachment(&depth_attachment);

                    // Record command buffer.
                    let command_buffer = vulkan.command_buffers[present_index as usize];
                    device
                        .begin_command_buffer(
                            command_buffer,
                            &vk::CommandBufferBeginInfo::builder()
                                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                        )
                        .context("Beginning command buffer")
                        .unwrap();
                    device.cmd_pipeline_barrier(
                        command_buffer,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        slice::from_ref(
                            &vk::ImageMemoryBarrier::builder()
                                .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                                .old_layout(vk::ImageLayout::UNDEFINED)
                                .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                                .image(vulkan.swapchain.images[present_index as usize].0)
                                .subresource_range(vk::ImageSubresourceRange {
                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                    base_mip_level: 0,
                                    level_count: 1,
                                    base_array_layer: 0,
                                    layer_count: 1,
                                }),
                        ),
                    );
                    device.cmd_pipeline_barrier(
                        command_buffer,
                        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        slice::from_ref(
                            &vk::ImageMemoryBarrier::builder()
                                .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
                                .old_layout(vk::ImageLayout::UNDEFINED)
                                .new_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                                .image(vulkan.depth.image)
                                .subresource_range(vk::ImageSubresourceRange {
                                    aspect_mask: vk::ImageAspectFlags::DEPTH,
                                    base_mip_level: 0,
                                    level_count: 1,
                                    base_array_layer: 0,
                                    layer_count: 1,
                                }),
                        ),
                    );
                    device.cmd_begin_rendering(command_buffer, &rendering_info);
                    device.cmd_set_viewport(
                        command_buffer,
                        0,
                        slice::from_ref(&vk::Viewport {
                            x: 0.0,
                            y: 0.0,
                            width: window_size.w as f32,
                            height: window_size.h as f32,
                            min_depth: 0.0,
                            max_depth: 1.0,
                        }),
                    );
                    device.cmd_set_scissor(
                        command_buffer,
                        0,
                        slice::from_ref(&vk::Rect2D {
                            offset: vk::Offset2D { x: 0, y: 0 },
                            extent: window_size.into(),
                        }),
                    );

                    let time = instant_start.elapsed().as_secs_f32();
                    vulkan
                        .scene
                        .draw(&vulkan.device, command_buffer, time, &assets_scene);

                    device.cmd_end_rendering(command_buffer);
                    device.cmd_pipeline_barrier(
                        command_buffer,
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        slice::from_ref(
                            &vk::ImageMemoryBarrier::builder()
                                .src_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)
                                .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                                .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                                .image(vulkan.swapchain.images[present_index as usize].0)
                                .subresource_range(vk::ImageSubresourceRange {
                                    aspect_mask: vk::ImageAspectFlags::COLOR,
                                    base_mip_level: 0,
                                    level_count: 1,
                                    base_array_layer: 0,
                                    layer_count: 1,
                                }),
                        ),
                    );
                    device
                        .end_command_buffer(command_buffer)
                        .context("Ending command buffer")
                        .unwrap();

                    // Submit.
                    let submit_info = vk::SubmitInfo::builder()
                        .wait_semaphores(slice::from_ref(
                            &vulkan.present_complete[frame_index as usize],
                        ))
                        .wait_dst_stage_mask(slice::from_ref(
                            &vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                        ))
                        .command_buffers(slice::from_ref(&command_buffer))
                        .signal_semaphores(slice::from_ref(
                            &vulkan.rendering_complete[frame_index as usize],
                        ));
                    device
                        .queue_submit(
                            *queue,
                            slice::from_ref(&submit_info),
                            vulkan.draw_commands_reuse[frame_index as usize],
                        )
                        .context("Submitting to queue")
                        .unwrap();

                    // Present.
                    let present_info = vk::PresentInfoKHR::builder()
                        .wait_semaphores(slice::from_ref(
                            &vulkan.rendering_complete[frame_index as usize],
                        ))
                        .swapchains(slice::from_ref(&vulkan.swapchain.handle))
                        .image_indices(slice::from_ref(&present_index));
                    let present_result = vulkan
                        .swapchain
                        .loader
                        .queue_present(*queue, &present_info)
                        .context("Presenting");
                    if present_result.is_err() || window_size != resized_window_size {
                        vulkan
                            .swapchain
                            .recreate(&vulkan.surface, &vulkan.device, resized_window_size.into())
                            .context("Recreating swapchain")
                            .unwrap();
                        vulkan
                            .depth
                            .recreate(&vulkan.device, resized_window_size.into())
                            .context("Recreating depth")
                            .unwrap();
                        window_size = resized_window_size;
                    }
                }

                frame_count += 1;
                frame_index = frame_count % u64::from(MAX_CONCURRENT_FRAMES);
            }

            _ => (),
        }
    });

    // Destroy Vulkan resources.
    unsafe {
        let device = &vulkan.device.handle;
        device
            .device_wait_idle()
            .context("Waiting for device idle")?;

        vulkan.scene.destroy(&vulkan.device);

        for i in 0..MAX_CONCURRENT_FRAMES {
            let i = i as usize;
            device.destroy_semaphore(vulkan.present_complete[i], None);
            device.destroy_semaphore(vulkan.rendering_complete[i], None);
            device.destroy_fence(vulkan.draw_commands_reuse[i], None);
        }

        device.free_command_buffers(vulkan.command_pool, &vulkan.command_buffers);
        device.destroy_command_pool(vulkan.command_pool, None);
        vulkan.depth.destroy(&vulkan.device);
        vulkan.swapchain.destroy(&vulkan.device);
        device.destroy_device(None);
        vulkan
            .surface
            .loader
            .destroy_surface(vulkan.surface.handle, None);
        if let Some(debug) = vulkan.debug {
            debug
                .utils
                .destroy_debug_utils_messenger(debug.callback, None);
        }
        vulkan.instance.destroy_instance(None);
    }

    Ok(())
}
