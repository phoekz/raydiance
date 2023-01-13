#![deny(future_incompatible)]
#![deny(nonstandard_style)]
#![deny(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
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
    time::{Duration, Instant},
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
    window::{Window, WindowBuilder},
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
mod window;

use aabb::*;
use ray::*;
use triangle::*;

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
    let min_window_size = window::Size { w: 320, h: 180 };
    let mut window_size = window::Size {
        w: 1280 / 4,
        h: 720 / 4,
    };
    let mut resized_window_size = window_size;
    let (mut event_loop, window) = window::create(&window::Params {
        title: window_title,
        size: window_size,
        min_size: min_window_size,
    })?;

    // Init GLB scene.
    let glb_scene = glb::Scene::create(include_bytes!("assets/rounded_cube.glb"))?;

    // Init raytracer.
    let raytracer = raytracing::Raytracer::create(
        raytracing::Params {
            samples_per_pixel: 64,
            ..raytracing::Params::default()
        },
        glb_scene.clone(),
    );

    // Init Vulkan renderer.
    let mut renderer =
        unsafe { vulkan::Renderer::create(&window, window_title, window_size, &glb_scene)? };

    // Main event loop.
    let mut current_time = Instant::now();
    let mut frame_index = 0_u64;
    let mut frame_count = 0_u64;
    let mut input_state = InputState::default();
    let mut camera_angle = 0.0;
    let mut camera_transform = na::Matrix4::identity();
    let mut display_raytracing_image = true;
    let mut hemisphere_sampler = sampling::HemisphereSampler::default();
    let mut sample_state = (0, 0);
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
                            if virtual_keycode == VirtualKeyCode::H {
                                if hemisphere_sampler == sampling::HemisphereSampler::Uniform {
                                    hemisphere_sampler = sampling::HemisphereSampler::Cosine;
                                } else {
                                    hemisphere_sampler = sampling::HemisphereSampler::Uniform;
                                }
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
                // Update window title.
                // Todo: replace with GUI.
                window.set_title(&format!(
                    "{window_title} - {hemisphere_sampler} - {}/{} samples",
                    sample_state.0, sample_state.1
                ));

                // Update clock.
                let delta_time = current_time.elapsed().as_secs_f32();
                current_time = Instant::now();

                // Update camera.
                let speed = TAU / 5.0;
                if input_state.a {
                    camera_angle -= speed * delta_time;
                }
                if input_state.d {
                    camera_angle += speed * delta_time;
                }
                camera_transform =
                    na::Matrix4::from_axis_angle(&na::Vector3::y_axis(), camera_angle);

                // Update raytracer.
                raytracer.send_input(raytracing::Input {
                    camera_transform,
                    image_size: (window_size.w, window_size.h),
                    hemisphere_sampler,
                });

                // Draw screen.
                window.request_redraw();
            }

            Event::RedrawRequested(_) => {
                unsafe {
                    if let Some(output) = raytracer.try_recv_output() {
                        renderer
                            .update_raytracing_image(&output.image, output.image_size)
                            .unwrap();
                        sample_state = (output.sample_index, output.sample_count);
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
    raytracer.terminate();
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
