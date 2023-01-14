use super::*;

pub struct DepthTarget {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
}

impl DepthTarget {
    pub unsafe fn create(device: &Device, window_size: vk::Extent2D) -> Result<Self> {
        // Image.
        let image = device.create_image(
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
                .samples(DEFAULT_SAMPLE_COUNT)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
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

    pub unsafe fn recreate(&mut self, device: &Device, window_size: vk::Extent2D) -> Result<()> {
        self.destroy(device);
        *self = Self::create(device, window_size)?;
        Ok(())
    }

    pub fn image(&self) -> vk::Image {
        self.image
    }

    pub fn image_view(&self) -> vk::ImageView {
        self.view
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.destroy_image_view(self.view, None);
        device.destroy_image(self.image, None);
        device.free_memory(self.memory, None);
    }
}
