use super::*;

mod buffer;
mod color_target;
mod debug;
mod depth_target;
mod device;
mod gui;
mod instance;
mod raster_scene;
mod raytracing_image;
mod shader;
mod surface;
mod swapchain;

use buffer::*;
use color_target::*;
use debug::*;
use depth_target::*;
use device::*;
use gui::*;
use instance::*;
use raster_scene::*;
use raytracing_image::*;
use shader::*;
use surface::*;
use swapchain::*;

const VULKAN_API_VERSION: u32 = vk::make_api_version(0, 1, 3, 0);
pub const MAX_CONCURRENT_FRAMES: u32 = 3;
const DEFAULT_DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;
const DEFAULT_SAMPLE_COUNT: vk::SampleCountFlags = vk::SampleCountFlags::TYPE_8;
const DEFAULT_PRESENT_MODE: vk::PresentModeKHR = vk::PresentModeKHR::FIFO;
const DEFAULT_SURFACE_COLOR_SPACE: vk::ColorSpaceKHR = vk::ColorSpaceKHR::SRGB_NONLINEAR;
const DEFAULT_SURFACE_FORMAT: vk::Format = vk::Format::B8G8R8A8_SRGB;

struct Commands {
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    present_complete: Vec<vk::Semaphore>,
    rendering_complete: Vec<vk::Semaphore>,
    draw_commands_reuse: Vec<vk::Fence>,
}

impl Commands {
    unsafe fn create(device: &Device) -> Result<Self> {
        let command_pool = device
            .create_command_pool(
                &vk::CommandPoolCreateInfo::builder()
                    .queue_family_index(device.queue().index())
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
            .context("Creating command pool")?;
        let command_buffers = device
            .allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::builder()
                    .command_buffer_count(MAX_CONCURRENT_FRAMES)
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY),
            )
            .context("Allocating command buffers")?;

        let mut present_complete = vec![];
        let mut rendering_complete = vec![];
        let mut draw_commands_reuse = vec![];
        for _ in 0..MAX_CONCURRENT_FRAMES {
            present_complete.push(
                device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .context("Creating semaphore")?,
            );
            rendering_complete.push(
                device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .context("Creating semaphore")?,
            );
            draw_commands_reuse.push(
                device
                    .create_fence(
                        &vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED),
                        None,
                    )
                    .context("Creating fence")?,
            );
        }

        Ok(Self {
            command_pool,
            command_buffers,
            present_complete,
            rendering_complete,
            draw_commands_reuse,
        })
    }

    unsafe fn destroy(&self, device: &Device) {
        for i in 0..MAX_CONCURRENT_FRAMES {
            let i = i as usize;
            device.destroy_semaphore(self.present_complete[i], None);
            device.destroy_semaphore(self.rendering_complete[i], None);
            device.destroy_fence(self.draw_commands_reuse[i], None);
        }
        device.free_command_buffers(self.command_pool, &self.command_buffers);
        device.destroy_command_pool(self.command_pool, None);
    }
}

pub struct Renderer {
    _entry: ash::Entry,
    instance: Instance,
    debug: Option<Debug>,
    surface: Surface,
    device: Device,
    swapchain: Swapchain,
    color_target: ColorTarget,
    depth_target: DepthTarget,
    gui: Gui,
    cmds: Commands,
    scene: RasterScene,
    rt_image: RaytracingImageRenderer,
}

impl Renderer {
    // Todo: Decouple swapchain from rendering and handle swapchain resizing and
    // minimizing logic separately from the application logic.

    pub unsafe fn create(
        window: &editor::Window,
        window_title: &str,
        window_size: editor::WindowSize,
        rds_scene: &rds::Scene,
        font_atlas_texture: &imgui::FontAtlasTexture,
    ) -> Result<Self> {
        let validation = std::env::var("VULKAN_VALIDATION").is_ok();
        validation.then(|| info!("Vulkan validation layers enabled"));
        let entry = unsafe { ash::Entry::load()? };
        let instance = Instance::create(&entry, validation, window_title)?;
        let debug = validation
            .then(|| Debug::create(&entry, &instance))
            .transpose()?;
        let surface = Surface::create(&entry, &instance, window.handle())?;
        let device = Device::create(&instance, &surface)?;
        let swapchain = Swapchain::create(&instance, &surface, &device, window_size.into())?;
        let color = ColorTarget::create(&device, window_size.into())?;
        let depth = DepthTarget::create(&device, window_size.into())?;
        let gui = Gui::create(&device, font_atlas_texture)?;
        let cmds = Commands::create(&device)?;
        let scene = RasterScene::create(&device, rds_scene)?;
        let rt_image = RaytracingImageRenderer::create(&device)?;
        Ok(Self {
            _entry: entry,
            instance,
            debug,
            surface,
            device,
            swapchain,
            color_target: color,
            depth_target: depth,
            gui,
            cmds,
            scene,
            rt_image,
        })
    }

    pub unsafe fn redraw(
        &mut self,
        dyn_scene: &rds::DynamicScene,
        window_size: editor::WindowSize,
        resized_window_size: editor::WindowSize,
        frame_index: u64,
        camera_transform: Mat4,
        display_raytracing_image: bool,
        visualize_normals: bool,
        gui_data: &imgui::DrawData,
    ) -> Result<()> {
        // Aliases.
        let queue = self.device.queue();
        let device = &self.device;
        let swapchain = &mut self.swapchain;
        let surface = &self.surface;
        let color_target = &mut self.color_target;
        let depth_target = &mut self.depth_target;
        let scene = &self.scene;
        let rt_image = &self.rt_image;
        let gui = &self.gui;
        let cmds = &self.cmds;
        let command_buffers = &cmds.command_buffers;
        let draw_commands_reuse = &cmds.draw_commands_reuse[frame_index as usize];
        let present_complete = &cmds.present_complete[frame_index as usize];
        let rendering_complete = &cmds.rendering_complete[frame_index as usize];

        // Stop rendering if the window is minimized (size equals to zero).
        if window_size.is_zero() || resized_window_size.is_zero() {
            return Ok(());
        }

        // Wait until previous frame is done.
        device
            .wait_for_fences(slice::from_ref(draw_commands_reuse), true, u64::MAX)
            .context("Waiting for fence")?;

        // Acquire image.
        let acquire_result = swapchain
            .loader()
            .acquire_next_image(**swapchain, u64::MAX, *present_complete, vk::Fence::null())
            .context("Acquiring next image");
        let Ok((present_index, _)) = acquire_result else {
            swapchain
                .recreate(surface, device, window_size.into())
                .context("Recreating swapchain")?;
            color_target
                .recreate(device, window_size.into())
                .context("Recreating color target")?;
            depth_target
                .recreate(device, window_size.into())
                .context("Recreating depth target")?;
            return Ok(());
        };

        // Synchronize previous frame.
        device
            .reset_fences(slice::from_ref(draw_commands_reuse))
            .context("Resetting fences")?;

        // Get latest present image.
        let present_image = swapchain.image(present_index);

        // Setup dynamic rendering.
        let color_attachment = vk::RenderingAttachmentInfo::builder()
            .image_view(color_target.image_view())
            .image_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
            .resolve_mode(vk::ResolveModeFlags::AVERAGE)
            .resolve_image_view(present_image.1)
            .resolve_image_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            });
        let depth_attachment = vk::RenderingAttachmentInfo::builder()
            .image_view(depth_target.image_view())
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
        let command_buffer = command_buffers[present_index as usize];
        device
            .begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )
            .context("Beginning command buffer")?;
        device.image_memory_barrier(
            command_buffer,
            depth_target.image(),
            vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS,
            vk::AccessFlags2::empty(),
            vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS,
            vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            vk::ImageAspectFlags::DEPTH,
        );
        device.image_memory_barrier(
            command_buffer,
            present_image.0,
            vk::PipelineStageFlags2::TOP_OF_PIPE,
            vk::AccessFlags2::empty(),
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageAspectFlags::COLOR,
        );
        device.cmd_begin_rendering(command_buffer, &rendering_info);
        device.cmd_set_viewport(
            command_buffer,
            0,
            slice::from_ref(
                // VK_KHR_maintenance1: Allow negative height to be specified in
                // the VkViewport::height field to perform y-inversion of the
                // clip-space to framebuffer-space transform. This allows apps
                // to avoid having to use gl_Position.y = -gl_Position.y in
                // shaders also targeting other APIs.
                &vk::Viewport {
                    x: 0.0,
                    y: window_size.h as f32,
                    width: window_size.w as f32,
                    height: -(window_size.h as f32),
                    min_depth: 0.0,
                    max_depth: 1.0,
                },
            ),
        );
        device.cmd_set_scissor(
            command_buffer,
            0,
            slice::from_ref(&vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: window_size.into(),
            }),
        );
        scene.draw(
            device,
            command_buffer,
            camera_transform,
            dyn_scene,
            visualize_normals,
        );
        if display_raytracing_image {
            rt_image.draw(device, command_buffer);
        }
        gui.draw(device, command_buffer, frame_index, gui_data);
        device.cmd_end_rendering(command_buffer);
        device.image_memory_barrier(
            command_buffer,
            present_image.0,
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
            vk::AccessFlags2::empty(),
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
            vk::ImageAspectFlags::COLOR,
        );
        device
            .end_command_buffer(command_buffer)
            .context("Ending command buffer")?;

        // Submit.
        device
            .queue_submit2(
                **queue,
                slice::from_ref(
                    &vk::SubmitInfo2::builder()
                        .wait_semaphore_infos(slice::from_ref(
                            &vk::SemaphoreSubmitInfo::builder()
                                .semaphore(*present_complete)
                                .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT),
                        ))
                        .command_buffer_infos(slice::from_ref(
                            &vk::CommandBufferSubmitInfo::builder().command_buffer(command_buffer),
                        ))
                        .signal_semaphore_infos(slice::from_ref(
                            &vk::SemaphoreSubmitInfo::builder()
                                .semaphore(*rendering_complete)
                                .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS),
                        )),
                ),
                *draw_commands_reuse,
            )
            .context("Submitting to queue")?;

        // Present.
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(slice::from_ref(rendering_complete))
            .swapchains(slice::from_ref(&*swapchain))
            .image_indices(slice::from_ref(&present_index));
        let present_result = swapchain
            .loader()
            .queue_present(**queue, &present_info)
            .context("Presenting");
        if present_result.is_err() || window_size != resized_window_size {
            swapchain
                .recreate(surface, device, resized_window_size.into())
                .context("Recreating swapchain")?;
            color_target
                .recreate(device, resized_window_size.into())
                .context("Recreating color target")?;
            depth_target
                .recreate(device, resized_window_size.into())
                .context("Recreating depth target")?;
        }

        Ok(())
    }

    pub unsafe fn update_raytracing_image(
        &mut self,
        raytracing_image: &[ColorRgb],
        raytracing_image_size: (u32, u32),
    ) -> Result<()> {
        self.rt_image
            .update(&self.device, raytracing_image, raytracing_image_size)?;
        Ok(())
    }

    pub unsafe fn update_gui(&self, frame_index: u64, gui_data: &imgui::DrawData) -> Result<()> {
        self.gui.update(&self.device, frame_index, gui_data)?;
        Ok(())
    }

    pub unsafe fn destroy(mut self) -> Result<()> {
        self.device
            .device_wait_idle()
            .context("Flushing pipeline")?;
        self.rt_image.destroy(&self.device);
        self.scene.destroy(&self.device);
        self.cmds.destroy(&self.device);
        self.gui.destroy(&self.device);
        self.color_target.destroy(&self.device);
        self.depth_target.destroy(&self.device);
        self.swapchain.destroy(&self.device);
        self.device.destroy();
        self.surface.destroy();
        if let Some(debug) = self.debug {
            debug.destroy();
        }
        self.instance.destroy();
        Ok(())
    }
}
