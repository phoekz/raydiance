use super::*;

pub struct Queue {
    handle: vk::Queue,
    index: u32,
}

impl Deref for Queue {
    type Target = vk::Queue;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl Queue {
    pub fn index(&self) -> u32 {
        self.index
    }
}

pub struct Device {
    handle: ash::Device,
    physical_device: vk::PhysicalDevice,
    queue: Queue,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    push_descriptor_khr: ash::extensions::khr::PushDescriptor,
}

impl Deref for Device {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl Device {
    pub unsafe fn create(instance: &Instance, surface: &Surface) -> Result<Self> {
        // Find physical device and its queues.
        let (physical_device, queue_family_index) = if let Some((physical_device, queue_families)) =
            instance
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
                    if features_13.synchronization2 == vk::FALSE {
                        return None;
                    }

                    // We only support discrete GPUs at this point.
                    let properties = instance.get_physical_device_properties(physical_device);
                    if properties.device_type != vk::PhysicalDeviceType::DISCRETE_GPU {
                        return None;
                    }

                    // Check limits.
                    let limits = properties.limits;
                    if !limits
                        .framebuffer_color_sample_counts
                        .contains(DEFAULT_SAMPLE_COUNT)
                    {
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
                    if let Ok(supports_present) = surface
                        .loader()
                        .get_physical_device_surface_support(physical_device, queue, **surface)
                    {
                        if !supports_present {
                            return None;
                        }
                    } else {
                        return None;
                    }

                    Some((physical_device, queue))
                }) {
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
                let check_support =
                    |extensions: &[vk::ExtensionProperties], extension_name: &CStr| -> Result<()> {
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

                let enabled_extensions =
                    [vk::KhrSwapchainFn::name(), vk::KhrPushDescriptorFn::name()];
                for extension in &enabled_extensions {
                    check_support(&extensions, extension)?;
                }
                enabled_extensions
                    .into_iter()
                    .map(CStr::as_ptr)
                    .collect::<Vec<_>>()
            };

            // Features.
            let mut features_13 = vk::PhysicalDeviceVulkan13Features::builder()
                .synchronization2(true)
                .dynamic_rendering(true);
            let mut features = vk::PhysicalDeviceFeatures2::builder().push_next(&mut features_13);

            // Create.
            instance
                .create_device(
                    physical_device,
                    &vk::DeviceCreateInfo::builder()
                        .queue_create_infos(&queue_create_infos)
                        .enabled_extension_names(&enabled_extensions)
                        .push_next(&mut features),
                    None,
                )
                .context("Creating device")?
        };

        // Memory properties.
        let memory_properties = instance.get_physical_device_memory_properties(physical_device);

        // Create queue.
        let queue = Queue {
            index: queue_family_index,
            handle: device.get_device_queue(queue_family_index, 0),
        };

        // Extension function pointers.
        let push_descriptor_khr = ash::extensions::khr::PushDescriptor::new(instance, &device);

        Ok(Self {
            handle: device,
            physical_device,
            queue,
            memory_properties,
            push_descriptor_khr,
        })
    }

    pub fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn push_descriptor_khr(&self) -> &ash::extensions::khr::PushDescriptor {
        &self.push_descriptor_khr
    }

    pub unsafe fn destroy(&self) {
        self.destroy_device(None);
    }

    pub unsafe fn find_memory_type_index(
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

    pub unsafe fn image_memory_barrier(
        &self,
        command_buffer: vk::CommandBuffer,
        image: vk::Image,
        src_stage_mask: vk::PipelineStageFlags2,
        src_access_mask: vk::AccessFlags2,
        dst_stage_mask: vk::PipelineStageFlags2,
        dst_access_mask: vk::AccessFlags2,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        aspect_mask: vk::ImageAspectFlags,
    ) {
        self.cmd_pipeline_barrier2(
            command_buffer,
            &vk::DependencyInfo::builder().image_memory_barriers(slice::from_ref(
                &vk::ImageMemoryBarrier2::builder()
                    .src_stage_mask(src_stage_mask)
                    .src_access_mask(src_access_mask)
                    .dst_stage_mask(dst_stage_mask)
                    .dst_access_mask(dst_access_mask)
                    .old_layout(old_layout)
                    .new_layout(new_layout)
                    .image(image)
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    }),
            )),
        );
    }
}
