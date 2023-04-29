use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    pub w: u32,
    pub h: u32,
}

impl WindowSize {
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.w == 0 && self.h == 0
    }
}

impl From<WindowSize> for winit::dpi::PhysicalSize<u32> {
    fn from(value: WindowSize) -> Self {
        Self {
            width: value.w,
            height: value.h,
        }
    }
}

impl From<WindowSize> for (u32, u32) {
    fn from(value: WindowSize) -> Self {
        (value.w, value.h)
    }
}

impl From<winit::dpi::PhysicalSize<u32>> for WindowSize {
    fn from(value: winit::dpi::PhysicalSize<u32>) -> Self {
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

pub struct Params<'a> {
    pub title: &'a str,
    pub size: WindowSize,
    pub min_size: WindowSize,
    pub decorations: bool,
}

pub struct Window {
    window: winit::window::Window,
    window_title: String,
    window_size: WindowSize,
    new_window_size: WindowSize,
}

impl Window {
    pub fn create(params: &Params) -> Result<(Self, winit::event_loop::EventLoop<()>)> {
        // Create event loop.
        let event_loop = winit::event_loop::EventLoop::new();

        // Build window.
        let window = winit::window::WindowBuilder::new()
            .with_title(params.title)
            .with_inner_size::<winit::dpi::PhysicalSize<_>>(params.size.into())
            .with_min_inner_size::<winit::dpi::PhysicalSize<_>>(params.min_size.into())
            .with_always_on_top(true)
            .with_resizable(true)
            .with_decorations(params.decorations)
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
        window.set_outer_position(winit::dpi::PhysicalPosition::new(
            (monitor_width - params.size.w) / 2,
            (monitor_height - params.size.h) / 2,
        ));

        Ok((
            Self {
                window,

                window_title: params.title.to_string(),
                window_size: params.size,
                new_window_size: params.min_size,
            },
            event_loop,
        ))
    }

    pub fn handle(&self) -> &winit::window::Window {
        &self.window
    }

    pub fn title(&self) -> &str {
        &self.window_title
    }

    pub fn size(&self) -> WindowSize {
        self.window_size
    }

    pub fn new_size(&self) -> WindowSize {
        self.new_window_size
    }

    pub fn handle_event(&mut self, event: &winit::event::Event<()>) {
        let winit::event::Event::WindowEvent { event, .. } = event else {
            return;
        };

        let winit::event::WindowEvent::Resized(new_window_size) = event else {
            return;
        };

        self.new_window_size = (*new_window_size).into();
    }

    // Call after Vulkan swapchain has handled the resize.
    pub fn handled_resize(&mut self) {
        self.window_size = self.new_window_size;
    }
}
