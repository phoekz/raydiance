#![deny(future_incompatible)]
#![deny(nonstandard_style)]
#![deny(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::wildcard_imports
)]

use std::{
    borrow::Cow,
    f32::consts::{PI, TAU},
    ffi::{CStr, CString},
    mem::{size_of, transmute},
    ops::Deref,
    slice,
    sync::mpsc,
    thread,
    time::Instant,
};

use anyhow::{anyhow, bail, ensure, Context, Result};
use ash::vk;
use bytemuck::{Pod, Zeroable};
use nalgebra as na;
use palette::{LinSrgb, LinSrgba, Pixel, Srgba, WithAlpha};
use rand::prelude::*;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

#[macro_use]
extern crate log;

mod aabb;
mod bvh;
mod glb;
mod intersection;
mod ray;
mod raytracing;
mod sampling;
mod triangle;
mod vulkan;

use aabb::*;
use ray::*;
use triangle::*;

//
// Window
//

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

impl From<WindowSize> for PhysicalSize<u32> {
    fn from(value: WindowSize) -> Self {
        Self {
            width: value.w,
            height: value.h,
        }
    }
}

impl From<PhysicalSize<u32>> for WindowSize {
    fn from(value: PhysicalSize<u32>) -> Self {
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

//
// Input state
//

#[derive(Default)]
struct InputState {
    a: bool,
    d: bool,
    t: bool,
}

//
// Main
//

fn main() -> Result<()> {
    // Init logging.
    env_logger::init();

    // Init winit.
    let window_title = env!("CARGO_PKG_NAME");
    let min_window_size = WindowSize { w: 320, h: 180 };
    let mut window_size = WindowSize {
        w: 1280 / 4,
        h: 720 / 4,
    };
    let mut resized_window_size = window_size;
    let (mut event_loop, window) = {
        // Create event loop.
        let event_loop = EventLoop::new();

        // Build window.
        let window = WindowBuilder::new()
            .with_title(window_title)
            .with_inner_size::<PhysicalSize<_>>(window_size.into())
            .with_min_inner_size::<PhysicalSize<_>>(min_window_size.into())
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
            (monitor_width - window_size.w) / 2,
            (monitor_height - window_size.h) / 2,
        ));

        (event_loop, window)
    };

    // Init scene.
    let assets_scene = glb::Scene::create(include_bytes!("assets/rounded_cube.glb"))?;

    // Init raytracer.
    let (raytracing_thread_tx, raytracing_thread_rx) = mpsc::channel();
    let (rendering_thread_tx, rendering_thread_rx) = mpsc::channel();
    let mut raytracing_is_busy = false;
    {
        let assets_scene = assets_scene.clone();
        thread::spawn(move || {
            let raytracing_scene = raytracing::Scene::create(&assets_scene);
            let raytracing_params = raytracing::RenderParameters {
                samples_per_pixel: 16,
                ..raytracing::RenderParameters::default()
            };
            let raytracing_image_size = (window_size.w, window_size.h);

            #[allow(clippy::while_let_loop)]
            loop {
                let camera_transform = match raytracing_thread_rx.recv() {
                    Ok(camera_transform) => camera_transform,
                    Err(_) => {
                        break;
                    }
                };
                let raytracing_image = raytracing::render(
                    &raytracing_params,
                    &raytracing_scene,
                    &assets_scene.cameras[0],
                    &camera_transform,
                    &assets_scene.materials,
                    raytracing_image_size,
                );
                if rendering_thread_tx
                    .send((raytracing_image, raytracing_image_size))
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    // Init Vulkan renderer.
    let mut renderer =
        unsafe { vulkan::Renderer::create(&window, window_title, window_size, &assets_scene)? };

    // Main event loop.
    let mut current_time = Instant::now();
    let mut frame_index = 0_u64;
    let mut frame_count = 0_u64;
    let mut input_state = InputState::default();
    let mut camera_angle = 0.0;
    let mut camera_transform = na::Matrix4::identity();
    let mut camera_changed = false;
    let mut display_raytracing_image = true;
    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        // Event handler.
        match event {
            // Close window if user hits the X.
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => {
                *control_flow = ControlFlow::Exit;
            }

            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                window_id,
            } if window_id == window.id() => {
                // Close window if user hits the escape key.
                if let KeyboardInput {
                    virtual_keycode: Some(VirtualKeyCode::Escape),
                    ..
                } = input
                {
                    *control_flow = ControlFlow::Exit;
                    return;
                }

                // Camera controls.
                if let Some(virtual_keycode) = input.virtual_keycode {
                    match input.state {
                        winit::event::ElementState::Pressed => {
                            if virtual_keycode == VirtualKeyCode::A {
                                input_state.a = true;
                            }
                            if virtual_keycode == VirtualKeyCode::D {
                                input_state.d = true;
                            }
                            if virtual_keycode == VirtualKeyCode::T {
                                input_state.t = true;
                                display_raytracing_image = !display_raytracing_image;
                            }
                        }
                        winit::event::ElementState::Released => {
                            if virtual_keycode == VirtualKeyCode::A {
                                input_state.a = false;
                            }
                            if virtual_keycode == VirtualKeyCode::D {
                                input_state.d = false;
                            }
                            if virtual_keycode == VirtualKeyCode::T {
                                input_state.t = false;
                            }
                        }
                    }
                }
            }

            Event::WindowEvent {
                event: WindowEvent::Resized(new_window_size),
                window_id,
            } if window_id == window.id() => {
                // Ignore the incorrect resized events at frame_count=0. Issue:
                // https://github.com/rust-windowing/winit/issues/2094
                debug!(
                    "New window size: {} x {}",
                    new_window_size.width, new_window_size.height
                );

                if frame_count == 0 {
                    debug!("Ignore resized event at frame_count={frame_count}");
                } else {
                    resized_window_size = new_window_size.into();
                }
            }

            Event::MainEventsCleared => {
                let delta_time = current_time.elapsed().as_secs_f32();

                if camera_changed && !raytracing_is_busy {
                    raytracing_thread_tx.send(camera_transform).unwrap();
                    raytracing_is_busy = true;
                    info!("Sent camera transform to raytracing thread");
                    camera_changed = false;
                }

                let speed = TAU / 5.0;
                if input_state.a {
                    camera_angle -= speed * delta_time;
                    camera_changed = true;
                }
                if input_state.d {
                    camera_angle += speed * delta_time;
                    camera_changed = true;
                }
                current_time = Instant::now();
                camera_transform =
                    na::Matrix4::from_axis_angle(&na::Vector3::y_axis(), camera_angle);

                window.request_redraw();
            }

            Event::RedrawRequested(_) => {
                unsafe {
                    match rendering_thread_rx.try_recv() {
                        Ok((image, image_size)) => {
                            renderer
                                .update_raytracing_image(&image, image_size)
                                .unwrap();
                            raytracing_is_busy = false;
                        }
                        Err(err) => match err {
                            mpsc::TryRecvError::Empty => {}
                            mpsc::TryRecvError::Disconnected => {
                                panic!("Raytracing thread channel disconnected")
                            }
                        },
                    }

                    renderer
                        .redraw(
                            window_size,
                            resized_window_size,
                            frame_index,
                            camera_transform,
                            display_raytracing_image,
                        )
                        .unwrap();
                }
                frame_count += 1;
                frame_index = frame_count % u64::from(vulkan::MAX_CONCURRENT_FRAMES);
                window_size = resized_window_size;
            }

            _ => (),
        }
    });

    // Cleanup.
    unsafe { renderer.destroy()? };

    Ok(())
}

#[allow(dead_code)]
fn save_image(
    image: &[LinSrgb],
    (image_width, image_height): (u32, u32),
    path: &str,
) -> Result<()> {
    let mut out_image = image::RgbaImage::new(image_width, image_height);
    image
        .iter()
        .zip(out_image.pixels_mut())
        .for_each(|(linear, dst)| {
            let linear = linear.with_alpha(1.0);
            let srgb = Srgba::from_linear(linear);
            let bytes: [u8; 4] = srgb.into_format().into_raw();
            *dst = image::Rgba(bytes);
        });
    Ok(out_image.save(path)?)
}
