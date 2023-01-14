use super::*;

pub struct Debug {
    utils: ash::extensions::ext::DebugUtils,
    callback: vk::DebugUtilsMessengerEXT,
}

impl Debug {
    pub unsafe fn create(entry: &ash::Entry, instance: &Instance) -> Result<Self> {
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

        let utils = ash::extensions::ext::DebugUtils::new(entry, instance);
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
            .context("Creating debug utils messenger")?;

        Ok(Self { utils, callback })
    }

    pub unsafe fn destroy(&self) {
        self.utils
            .destroy_debug_utils_messenger(self.callback, None);
    }
}
