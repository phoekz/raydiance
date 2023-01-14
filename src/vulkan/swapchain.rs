use super::*;

pub struct Swapchain {
    handle: vk::SwapchainKHR,
    loader: ash::extensions::khr::Swapchain,
    images: Vec<(vk::Image, vk::ImageView)>,
}

impl Deref for Swapchain {
    type Target = vk::SwapchainKHR;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl Swapchain {
    const PRE_TRANSFORM: vk::SurfaceTransformFlagsKHR = vk::SurfaceTransformFlagsKHR::IDENTITY;
    const COMPOSITE_TRANSFORM: vk::CompositeAlphaFlagsKHR = vk::CompositeAlphaFlagsKHR::OPAQUE;
    const IMAGE_USAGE: vk::ImageUsageFlags = vk::ImageUsageFlags::COLOR_ATTACHMENT;

    pub unsafe fn create(
        instance: &ash::Instance,
        surface: &Surface,
        device: &Device,
        window_size: vk::Extent2D,
    ) -> Result<Self> {
        // Validate surface format.
        {
            let surface_formats = surface
                .loader()
                .get_physical_device_surface_formats(device.physical_device(), **surface)
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
                .loader()
                .get_physical_device_surface_capabilities(device.physical_device(), **surface)
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
                .loader()
                .get_physical_device_surface_present_modes(device.physical_device(), **surface)
                .context("Getting surface present modes")?;
            if !surface_present_mode.contains(&DEFAULT_PRESENT_MODE) {
                bail!("Surface must support {DEFAULT_PRESENT_MODE:?}");
            }
        }

        // Create loader.
        let loader = ash::extensions::khr::Swapchain::new(instance, device);

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

    pub fn loader(&self) -> &ash::extensions::khr::Swapchain {
        &self.loader
    }

    pub fn image(&self, image: u32) -> &(vk::Image, vk::ImageView) {
        &self.images[image as usize]
    }

    pub unsafe fn destroy(&mut self, device: &Device) {
        self.images.iter().for_each(|&images| {
            device.destroy_image_view(images.1, None);
        });
        self.images.clear();
        if self.handle != vk::SwapchainKHR::null() {
            self.loader.destroy_swapchain(self.handle, None);
        }
    }

    pub unsafe fn recreate(
        &mut self,
        surface: &Surface,
        device: &Device,
        window_size: vk::Extent2D,
    ) -> Result<()> {
        // Flush pipeline.
        device
            .device_wait_idle()
            .context("Waiting for device idle")?;

        // Destroy old swapchain.
        self.destroy(device);

        // Create new swapchain.
        let swapchain = self
            .loader
            .create_swapchain(
                &vk::SwapchainCreateInfoKHR::builder()
                    .surface(**surface)
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
