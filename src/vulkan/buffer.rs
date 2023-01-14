use super::*;

pub struct Buffer {
    handle: vk::Buffer,
    memory: vk::DeviceMemory,
    byte_count: usize,
}

impl Deref for Buffer {
    type Target = vk::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl Buffer {
    pub unsafe fn create(
        device: &Device,
        usage_flags: vk::BufferUsageFlags,
        property_flags: vk::MemoryPropertyFlags,
        byte_count: usize,
        bytes: &[u8],
    ) -> Result<Self> {
        // Create initial buffer.
        let buffer = device
            .create_buffer(
                &vk::BufferCreateInfo::builder()
                    .size(byte_count as u64)
                    .usage(usage_flags)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            )
            .with_context(|| format!("Creating buffer of bytes={byte_count}"))?;

        // Find memory type index.
        let requirements = device.get_buffer_memory_requirements(buffer);
        let index = device.find_memory_type_index(property_flags, requirements)?;

        // Create allocation.
        let allocate_info = vk::MemoryAllocateInfo::builder()
            .allocation_size(requirements.size)
            .memory_type_index(index)
            .build();
        let memory = device.allocate_memory(&allocate_info, None)?;

        // Copy to staging buffer.
        if !bytes.is_empty() {
            let map_flags = vk::MemoryMapFlags::empty();
            let ptr = device.map_memory(memory, 0, requirements.size, map_flags)?;
            let ptr = ptr.cast::<u8>();
            let mapped_slice = std::slice::from_raw_parts_mut(ptr, byte_count);
            mapped_slice.copy_from_slice(bytes);
            device.unmap_memory(memory);
        }

        // Bind memory.
        device.bind_buffer_memory(buffer, memory, 0)?;

        Ok(Self {
            handle: buffer,
            memory,
            byte_count,
        })
    }

    pub unsafe fn create_init<T: Copy + Zeroable + Pod>(
        device: &Device,
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

    pub unsafe fn copy_to(&self, device: &Device, dst: &Self) -> Result<()> {
        // Validate.
        if self.byte_count != dst.byte_count {
            bail!(
                "src and dst must have the same size, got src={} and dst={} instead",
                self.byte_count,
                dst.byte_count
            );
        }

        // Create temporary upload setup.
        let pool = device.create_command_pool(
            &vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(device.queue().index()),
            None,
        )?;
        let cmd = device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::builder()
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1),
        )?[0];
        let fence = device.create_fence(
            &vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::empty()),
            None,
        )?;

        // Record commands.
        device.begin_command_buffer(cmd, &vk::CommandBufferBeginInfo::builder())?;
        device.cmd_copy_buffer(
            cmd,
            self.handle,
            dst.handle,
            &[vk::BufferCopy {
                src_offset: 0,
                dst_offset: 0,
                size: self.byte_count as u64,
            }],
        );
        device.end_command_buffer(cmd)?;

        // Submit and wait.
        device.queue_submit(
            **device.queue(),
            &[*vk::SubmitInfo::builder().command_buffers(&[cmd])],
            fence,
        )?;
        device.wait_for_fences(&[fence], true, u64::MAX)?;

        // Cleanup.
        device.destroy_fence(fence, None);
        device.free_command_buffers(pool, &[cmd]);
        device.destroy_command_pool(pool, None);

        Ok(())
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.destroy_buffer(self.handle, None);
        device.free_memory(self.memory, None);
    }
}
