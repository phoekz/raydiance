#![deny(future_incompatible)]
#![deny(nonstandard_style)]
#![deny(clippy::pedantic)]
#![allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use std::{
    borrow::Cow,
    ffi::{CStr, CString},
    slice,
};

use anyhow::{bail, Context, Result};
use ash::{
    extensions::khr::{Surface, Swapchain},
    vk::{self, SwapchainCreateInfoKHR},
};
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

//
// Vulkan
//

const VULKAN_API_VERSION: u32 = vk::make_api_version(0, 1, 3, 0);
const MAX_CONCURRENT_FRAMES: u32 = 3;
const DEFAULT_SURFACE_FORMAT: vk::Format = vk::Format::B8G8R8A8_SRGB;
const DEFAULT_SURFACE_COLOR_SPACE: vk::ColorSpaceKHR = vk::ColorSpaceKHR::SRGB_NONLINEAR;
const DEFAULT_PRESENT_MODE: vk::PresentModeKHR = vk::PresentModeKHR::FIFO;

struct VulkanDebug {
    utils: ash::extensions::ext::DebugUtils,
    callback: vk::DebugUtilsMessengerEXT,
}

struct VulkanSurface {
    handle: vk::SurfaceKHR,
    loader: ash::extensions::khr::Surface,
}

struct VulkanQueue {
    index: u32,
    queue: vk::Queue,
}

struct VulkanDevice {
    handle: ash::Device,
    physical_device: vk::PhysicalDevice,
    queue: VulkanQueue,
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

    unsafe fn new(
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

struct VulkanContext {
    instance: ash::Instance,
    debug: Option<VulkanDebug>,
    surface: VulkanSurface,
    device: VulkanDevice,
    swapchain: VulkanSwapchain,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    present_complete: Vec<vk::Semaphore>,
    rendering_complete: Vec<vk::Semaphore>,
    draw_commands_reuse: Vec<vk::Fence>,
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
    let mut window_size = WindowSize { w: 1280, h: 720 };
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

            // Create queue.
            let queue = VulkanQueue {
                index: queue_family_index,
                queue: device.get_device_queue(queue_family_index, 0),
            };

            VulkanDevice {
                handle: device,
                physical_device,
                queue,
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

        let swapchain = VulkanSwapchain::new(&instance, &surface, &device, window_size.into())
            .context("Creating swapchain")?;

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

        VulkanContext {
            instance,
            debug,
            surface,
            device,
            swapchain,
            command_pool,
            command_buffers,
            present_complete,
            rendering_complete,
            draw_commands_reuse,
        }
    };

    // Main event loop.
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
                    let queue = &vulkan.device.queue.queue;

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
                    let color_attachment_info = vk::RenderingAttachmentInfo::builder()
                        .image_view(vulkan.swapchain.images[present_index as usize].1)
                        .image_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
                        .load_op(vk::AttachmentLoadOp::CLEAR)
                        .store_op(vk::AttachmentStoreOp::STORE)
                        .clear_value(vk::ClearValue {
                            color: vk::ClearColorValue {
                                float32: [rgb.red, rgb.green, rgb.blue, 1.0],
                            },
                        });
                    let rendering_info = vk::RenderingInfo::builder()
                        .render_area(vk::Rect2D {
                            offset: vk::Offset2D::default(),
                            extent: window_size.into(),
                        })
                        .layer_count(1)
                        .color_attachments(slice::from_ref(&color_attachment_info));

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
                    device.cmd_begin_rendering(command_buffer, &rendering_info);
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
        for i in 0..MAX_CONCURRENT_FRAMES {
            let i = i as usize;
            device.destroy_semaphore(vulkan.present_complete[i], None);
            device.destroy_semaphore(vulkan.rendering_complete[i], None);
            device.destroy_fence(vulkan.draw_commands_reuse[i], None);
        }

        device.free_command_buffers(vulkan.command_pool, &vulkan.command_buffers);
        device.destroy_command_pool(vulkan.command_pool, None);
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
