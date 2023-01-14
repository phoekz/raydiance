use super::*;

pub struct Instance {
    handle: ash::Instance,
}

impl Deref for Instance {
    type Target = ash::Instance;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl Instance {
    pub unsafe fn create(
        entry: &ash::Entry,
        validation: bool,
        window_title: &str,
    ) -> anyhow::Result<Self> {
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
            let check_support = |layers: &[vk::LayerProperties], layer_name: &CStr| -> Result<()> {
                if layers
                    .iter()
                    .any(|layer| CStr::from_ptr(layer.layer_name.as_ptr()) == layer_name)
                {
                    return Ok(());
                }
                bail!("Instance must support layer={layer_name:?}");
            };

            let layers = entry
                .enumerate_instance_layer_properties()
                .context("Getting instance layers")?;
            debug!("{layers:#?}");

            let khronos_validation = CStr::from_bytes_with_nul(b"VK_LAYER_KHRONOS_validation\0")?;
            check_support(&layers, khronos_validation)?;
            let mut enabled_layers = vec![];
            if validation {
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

            let extensions = entry
                .enumerate_instance_extension_properties(None)
                .context("Getting instance extensions")?;
            debug!("{extensions:#?}");

            let mut enabled_extensions = vec![];
            enabled_extensions.push(vk::KhrSurfaceFn::name());
            enabled_extensions.push(vk::KhrWin32SurfaceFn::name());
            if validation {
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
        let instance = entry
            .create_instance(
                &vk::InstanceCreateInfo::builder()
                    .application_info(&application_info)
                    .enabled_layer_names(&enabled_layers)
                    .enabled_extension_names(&enabled_extensions),
                None,
            )
            .context("Creating instance")?;

        Ok(Self { handle: instance })
    }

    pub unsafe fn destroy(&self) {
        self.destroy_instance(None);
    }
}
