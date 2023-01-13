use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

impl Size {
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.w == 0 && self.h == 0
    }
}

impl From<Size> for PhysicalSize<u32> {
    fn from(value: Size) -> Self {
        Self {
            width: value.w,
            height: value.h,
        }
    }
}

impl From<PhysicalSize<u32>> for Size {
    fn from(value: PhysicalSize<u32>) -> Self {
        Self {
            w: value.width,
            h: value.height,
        }
    }
}

impl From<Size> for vk::Extent2D {
    fn from(value: Size) -> Self {
        Self {
            width: value.w,
            height: value.h,
        }
    }
}

pub struct Params<'a> {
    pub title: &'a str,
    pub size: Size,
    pub min_size: Size,
}

pub fn create(params: &Params) -> Result<(EventLoop<()>, Window)> {
    // Create event loop.
    let event_loop = EventLoop::new();

    // Build window.
    let window = WindowBuilder::new()
        .with_title(params.title)
        .with_inner_size::<PhysicalSize<_>>(params.size.into())
        .with_min_inner_size::<PhysicalSize<_>>(params.min_size.into())
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
        (monitor_width - params.size.w) / 2,
        (monitor_height - params.size.h) / 2,
    ));

    Ok((event_loop, window))
}
