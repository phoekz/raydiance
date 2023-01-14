use super::*;

pub struct Shader {
    handle: vk::ShaderModule,
}

impl Deref for Shader {
    type Target = vk::ShaderModule;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl Shader {
    pub unsafe fn create(device: &Device, bytes: &[u8]) -> Result<Self> {
        let dwords = bytes
            .chunks_exact(4)
            .map(|chunk| transmute([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect::<Vec<u32>>();
        let module = device
            .create_shader_module(&vk::ShaderModuleCreateInfo::builder().code(&dwords), None)
            .context("Compiling shader")?;
        Ok(Self { handle: module })
    }

    pub unsafe fn destroy(&self, device: &Device) {
        device.destroy_shader_module(self.handle, None);
    }
}
