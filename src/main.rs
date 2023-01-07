use anyhow::{Context, Result};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
#[macro_use]
extern crate log;

fn main() -> Result<()> {
    // Initialize logger.
    env_logger::init();

    // Create event loop.
    let event_loop = EventLoop::new();

    // Build window.
    let (min_window_width, min_window_height) = (320, 180);
    let (window_width, window_height) = (1280, 720);
    let window = WindowBuilder::new()
        .with_title(env!("CARGO_PKG_NAME"))
        .with_inner_size(PhysicalSize {
            width: window_width,
            height: window_height,
        })
        .with_min_inner_size(PhysicalSize {
            width: min_window_width,
            height: min_window_height,
        })
        .with_always_on_top(true)
        .build(&event_loop)
        .context("Failed to build winit window.")?;

    // Get primary monitor dimensions.
    let (monitor_width, monitor_height) = {
        let monitor = window
            .primary_monitor()
            .context("Failed to get primary monitor.")?;
        (monitor.size().width, monitor.size().height)
    };
    info!("Primary monitor dimensions: {monitor_width} x {monitor_height}");

    // Center window.
    window.set_outer_position(PhysicalPosition::new(
        (monitor_width - window_width) / 2,
        (monitor_height - window_height) / 2,
    ));

    // Main event loop.
    let mut frame_index = 0_u64;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        // Event handler.
        match event {
            // Close window if user hits the X.
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,

            // Close window if user hits the escape key.
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    },
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,

            // Window changed size.
            Event::WindowEvent {
                event:
                    WindowEvent::Resized(PhysicalSize {
                        width: new_window_width,
                        height: new_window_height,
                    }),
                window_id,
            } if window_id == window.id() => {
                // Ignore the incorrect resized events at frame_index=0. Issue:
                // https://github.com/rust-windowing/winit/issues/2094
                info!("New window size: {new_window_width} x {new_window_height}");

                if frame_index == 0 {
                    warn!("Ignore resized event at frame_index={frame_index}.");
                } else {
                    // Todo: Handle resize.
                }
            }

            // Redraw.
            Event::RedrawRequested(_) => {
                // Todo: Issue GPU commands.
            }

            // End of frame.
            Event::MainEventsCleared => {
                // Increment frame index.
                frame_index += 1;
            }

            _ => (),
        }
    });
}
