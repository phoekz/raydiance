use super::*;

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

pub struct Surface {
    handle: vk::SurfaceKHR,
    loader: ash::extensions::khr::Surface,
}

impl Deref for Surface {
    type Target = vk::SurfaceKHR;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl Surface {
    pub unsafe fn create(
        entry: &ash::Entry,
        instance: &Instance,
        window: &winit::window::Window,
    ) -> Result<Self> {
        let surface = ash_window::create_surface(
            entry,
            instance,
            window.raw_display_handle(),
            window.raw_window_handle(),
            None,
        )
        .context("Creating surface")?;
        let loader = ash::extensions::khr::Surface::new(entry, instance);

        Ok(Self {
            handle: surface,
            loader,
        })
    }

    pub fn loader(&self) -> &ash::extensions::khr::Surface {
        &self.loader
    }

    pub unsafe fn destroy(&self) {
        self.loader.destroy_surface(self.handle, None);
    }
}
