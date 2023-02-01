#![deny(future_incompatible)]
#![deny(nonstandard_style)]
#![deny(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::collapsible_if,
    clippy::many_single_char_names,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::wildcard_imports
)]

use std::{
    borrow::Cow,
    collections::VecDeque,
    ffi::{CStr, CString},
    fs::File,
    io::BufWriter,
    mem::{size_of, transmute},
    ops::Deref,
    path::{Path, PathBuf},
    slice,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, ensure, Context, Result};
use ash::vk;
use bitvec::prelude::*;
use bytemuck::{Pod, Zeroable};
use clap::{Parser, Subcommand};
use nalgebra as na;
use rand::prelude::*;
use rayon::prelude::*;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::{Window, WindowBuilder},
};

#[macro_use]
extern crate log;

mod blog;
mod cpupt;
mod debug;
mod glb;
mod gui;
mod math;
mod vulkan;
mod window;

use cpupt::bxdfs;
use cpupt::sampling::{HemisphereSampler, UniformSampler};
use math::*;

const PI: f32 = std::f32::consts::PI;
const TAU: f32 = std::f32::consts::TAU;
const INV_PI: f32 = std::f32::consts::FRAC_1_PI;

//
// Input state
//

#[derive(Default)]
struct InputState {
    a: bool,
    d: bool,
}

//
// Timing
//

struct DeltaTimes {
    buffer: VecDeque<f32>,
    current_time: f32,
    trigger_time: f32,
    display_avg_fps: f32,
}

impl DeltaTimes {
    const MAX_SIZE: usize = 4;

    fn new(trigger_time: f32) -> DeltaTimes {
        Self {
            buffer: VecDeque::with_capacity(Self::MAX_SIZE),
            current_time: 0.0,
            trigger_time,
            display_avg_fps: 0.0,
        }
    }

    fn push(&mut self, delta_time: f32) {
        // Update ring buffer.
        self.buffer.push_front(delta_time);
        if self.buffer.len() > Self::MAX_SIZE {
            self.buffer.pop_back();
        }

        // Update (slowed down) display times.
        self.current_time += delta_time;
        if self.current_time > self.trigger_time {
            self.display_avg_fps = self.avg_fps();
            self.current_time = 0.0;
        }
    }

    fn avg_fps(&self) -> f32 {
        let mut sum = 0.0;
        for &delta_time in &self.buffer {
            sum += delta_time.recip();
        }
        sum / self.buffer.len() as f32
    }

    fn display_text(&self) -> String {
        format!("FPS: {:.03}", self.display_avg_fps)
    }
}

//
// Main
//

#[derive(Parser)]
#[clap(author, version)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Editor,
    Debug,
    Blog,
}

impl Default for Commands {
    fn default() -> Self {
        Self::Editor
    }
}

fn main() -> Result<()> {
    // Init logging.
    env_logger::init();

    // Execute command.
    match Args::parse().command {
        Commands::Editor => editor(),
        Commands::Debug => debug::run(),
        Commands::Blog => blog::build(),
    }
}

//
// Editor
//

fn editor() -> Result<()> {
    // Init winit.
    let window_title = env!("CARGO_PKG_NAME");
    let window_aspect = (16, 9);
    let window_aspect_multi = 50;
    let min_window_size = window::Size { w: 320, h: 180 };
    let mut window_size = window::Size {
        w: window_aspect.0 * window_aspect_multi,
        h: window_aspect.1 * window_aspect_multi,
    };
    let mut resized_window_size = window_size;
    let (mut event_loop, window) = window::create(&window::Params {
        title: window_title,
        size: window_size,
        min_size: min_window_size,
        decorations: true,
    })?;

    // Init gui.
    let mut gui = gui::Gui::create(&window);

    // Init glb scene.
    let (glb_scene, mut dyn_scene) = glb::Scene::create(include_bytes!("assets/rounded_cube.glb"))?;

    // Init cpupt.
    let raytracer = cpupt::Raytracer::create(
        cpupt::Params {
            samples_per_pixel: 256,
            ..cpupt::Params::default()
        },
        glb_scene.clone(),
    );

    // Init Vulkan renderer.
    let mut renderer = unsafe {
        vulkan::Renderer::create(
            &window,
            window_title,
            window_size,
            &glb_scene,
            &gui.font_atlas_texture(),
        )?
    };

    // Main event loop.
    let mut prev_time = Instant::now();
    let mut delta_time = 0.0;
    let mut delta_times = DeltaTimes::new(0.25);
    let mut frame_index = 0_u64;
    let mut frame_count = 0_u64;
    let mut input_state = InputState::default();
    let mut camera_angle = 0.0;
    let mut camera_transform = na::Matrix4::identity();
    let mut display_raytracing_image = true;
    let mut hemisphere_sampler = HemisphereSampler::default();
    let mut latest_output: Option<cpupt::Output> = None;
    let mut sample_state = (0, 0);
    let mut selected_material = 0;
    let mut visualize_normals = false;
    let mut any_window_focused = false;
    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        // Event handler.
        gui.handle_event(&window, &event);
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

                // Ignore app keybindings if any GUI window is focused.
                if any_window_focused {
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
                        }
                        winit::event::ElementState::Released => {
                            if virtual_keycode == VirtualKeyCode::A {
                                input_state.a = false;
                            }
                            if virtual_keycode == VirtualKeyCode::D {
                                input_state.d = false;
                            }
                        }
                    }
                }
            }

            Event::WindowEvent {
                event: WindowEvent::Resized(new_window_size),
                window_id,
            } if window_id == window.id() => {
                debug!(
                    "New window size: {} x {}",
                    new_window_size.width, new_window_size.height
                );
                resized_window_size = new_window_size.into();
            }

            Event::NewEvents(_) => {
                // Update clock.
                let delta_duration = prev_time.elapsed();
                delta_time = delta_duration.as_secs_f32();
                delta_times.push(delta_time);
                prev_time = Instant::now();

                // Update gui.
                gui.update_delta_time(delta_duration);
            }

            Event::MainEventsCleared => {
                // Update gui.
                gui.prepare_frame(&window);

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
                raytracer.send_input(cpupt::Input {
                    camera_transform,
                    image_size: (window_size.w, window_size.h),
                    hemisphere_sampler,
                    dyn_scene: dyn_scene.clone(),
                    visualize_normals,
                });

                // Draw screen.
                window.request_redraw();
            }

            Event::RedrawRequested(_) => {
                gui.frame(&window, |ui| {
                    // Update state.
                    any_window_focused =
                        ui.is_window_focused_with_flags(imgui::WindowFocusedFlags::ANY_WINDOW);

                    // Main window.
                    ui.window("Raydiance")
                        .size([360.0, 200.0], imgui::Condition::FirstUseEver)
                        .position_pivot([0.0, 0.0])
                        .position([0.0, 0.0], imgui::Condition::FirstUseEver)
                        .collapsible(true)
                        .resizable(false)
                        .movable(false)
                        .build(|| {
                            let style = ui.clone_style();

                            // Performance counters.
                            {
                                ui.text(delta_times.display_text());
                            }

                            // Rendering status.
                            {
                                let progress = sample_state.0 as f32 / sample_state.1 as f32;
                                imgui::ProgressBar::new(progress).size([0.0, 0.0]).build(ui);
                                ui.same_line();
                                ui.same_line_with_spacing(0.0, style.item_inner_spacing[0]);
                                ui.text("Rendering");
                            }

                            // Material editor.
                            {
                                // Selector.
                                ui.combo(
                                    "Material",
                                    &mut selected_material,
                                    &glb_scene.materials,
                                    |material| Cow::Borrowed(&material.name),
                                );

                                let material = &mut dyn_scene.materials[selected_material];

                                // Material model.
                                {
                                    let model = &mut material.model;
                                    if let Some(token) = ui.begin_combo("Model", model.name()) {
                                        if ui.selectable(glb::MaterialModel::Diffuse.name()) {
                                            *model = glb::MaterialModel::Diffuse;
                                        }
                                        if ui.selectable(glb::MaterialModel::Disney.name()) {
                                            *model = glb::MaterialModel::Disney;
                                        }
                                        token.end();
                                    }
                                }

                                // Texture editor.
                                {
                                    let _id = ui.push_id("Base color");

                                    let index = material.base_color as usize;
                                    let mut texture = &mut dyn_scene.textures[index];
                                    let mut bit = dyn_scene.replaced_textures[index];

                                    ui.text("Base color");
                                    if let glb::DynamicTexture::Vector4(ref mut v) = &mut texture {
                                        if ui.color_edit4("Value", v) {
                                            // Convenience: replace texture when an edit has been made without extra interaction.
                                            dyn_scene.replaced_textures.set(index, true);
                                        }
                                    }
                                    if ui.checkbox("Use", &mut bit) {
                                        dyn_scene.replaced_textures.set(index, bit);
                                    }
                                    ui.same_line();
                                    if ui.button("Reset") {
                                        // Convenience: reset to default value and clear replacement with one click.
                                        *texture = dyn_scene.default_textures[index];
                                        dyn_scene.replaced_textures.set(index, false);
                                    }
                                }
                                {
                                    let _id = ui.push_id("Roughness");

                                    let index = material.roughness as usize;
                                    let mut texture = &mut dyn_scene.textures[index];
                                    let mut bit = dyn_scene.replaced_textures[index];

                                    ui.text("Roughness");
                                    if let glb::DynamicTexture::Scalar(ref mut s) = &mut texture {
                                        if ui.slider("Value", 0.0, 1.0, s) {
                                            // Convenience: replace texture when an edit has been made without extra interaction.
                                            dyn_scene.replaced_textures.set(index, true);
                                        }
                                    }
                                    if ui.checkbox("Use", &mut bit) {
                                        dyn_scene.replaced_textures.set(index, bit);
                                    }
                                    ui.same_line();
                                    if ui.button("Reset") {
                                        // Convenience: reset to default value and clear replacement with one click.
                                        *texture = dyn_scene.default_textures[index];
                                        dyn_scene.replaced_textures.set(index, false);
                                    }
                                }
                                {
                                    let _id = ui.push_id("Metallic");

                                    let index = material.metallic as usize;
                                    let mut texture = &mut dyn_scene.textures[index];
                                    let mut bit = dyn_scene.replaced_textures[index];

                                    ui.text("Metallic");
                                    if let glb::DynamicTexture::Scalar(ref mut s) = &mut texture {
                                        if ui.slider("Value", 0.0, 1.0, s) {
                                            // Convenience: replace texture when an edit has been made without extra interaction.
                                            dyn_scene.replaced_textures.set(index, true);
                                        }
                                    }
                                    if ui.checkbox("Use", &mut bit) {
                                        dyn_scene.replaced_textures.set(index, bit);
                                    }
                                    ui.same_line();
                                    if ui.button("Reset") {
                                        // Convenience: reset to default value and clear replacement with one click.
                                        *texture = dyn_scene.default_textures[index];
                                        dyn_scene.replaced_textures.set(index, false);
                                    }
                                }
                            }

                            ui.checkbox("Visualize normals", &mut visualize_normals);

                            // Rendering config.
                            if let Some(token) =
                                ui.begin_combo("Hemisphere sampler", hemisphere_sampler.name())
                            {
                                if ui.selectable(HemisphereSampler::Uniform.name()) {
                                    hemisphere_sampler = HemisphereSampler::Uniform;
                                }

                                if ui.selectable(HemisphereSampler::Cosine.name()) {
                                    hemisphere_sampler = HemisphereSampler::Cosine;
                                }

                                token.end();
                            }

                            // Image utilities.
                            ui.checkbox("Show raytracing image", &mut display_raytracing_image);
                            if ui.button("Save image") {
                                use time::format_description;
                                use time::OffsetDateTime;

                                if let Some(output) = &latest_output {
                                    let local_time = OffsetDateTime::now_local()
                                        .expect("Failed to get local time");
                                    let format = format_description::parse(
                                        "[year][month][day]-[hour][minute][second]",
                                    )
                                    .expect("Failed to parse format description");
                                    let timestamp = local_time
                                        .format(&format)
                                        .expect("Failed to format local time");
                                    let path = format!("{timestamp}.png");
                                    save_image(&output.image, output.image_size, &path)
                                        .unwrap_or_else(|_| {
                                            panic!("Failed to save image to {path}")
                                        });
                                }
                            }
                        });
                });

                unsafe {
                    if let Some(output) = raytracer.try_recv_output() {
                        renderer
                            .update_raytracing_image(&output.image, output.image_size)
                            .unwrap();
                        sample_state = (output.sample_index, output.sample_count);
                        latest_output = Some(output);
                    }

                    let gui_data = gui.render();
                    renderer.update_gui(frame_index, gui_data).unwrap();

                    renderer
                        .redraw(
                            &dyn_scene,
                            window_size,
                            resized_window_size,
                            frame_index,
                            camera_transform,
                            display_raytracing_image,
                            visualize_normals,
                            gui_data,
                        )
                        .unwrap();
                }
                frame_count += 1;
                frame_index = frame_count % u64::from(vulkan::MAX_CONCURRENT_FRAMES);
                window_size = resized_window_size;
            }

            _ => {}
        }
    });

    // Cleanup.
    raytracer.terminate();
    unsafe { renderer.destroy()? };

    Ok(())
}

//
// Misc
//

#[allow(dead_code)]
fn save_image(
    image: &[ColorRgb],
    (image_width, image_height): (u32, u32),
    path: &str,
) -> Result<()> {
    use palette::{LinSrgba, Pixel, Srgba};
    let mut out_image = image::RgbaImage::new(image_width, image_height);
    image
        .iter()
        .zip(out_image.pixels_mut())
        .for_each(|(linear, dst)| {
            let linear = LinSrgba::new(linear.red(), linear.green(), linear.blue(), 1.0);
            let srgb = Srgba::from_linear(linear);
            let bytes: [u8; 4] = srgb.into_format().into_raw();
            *dst = image::Rgba(bytes);
        });
    info!("Saving image to {path}");
    Ok(out_image.save(path)?)
}

#[allow(dead_code)]
fn create_checkerboard_texture() {
    let w = 32;
    let h = 32;
    let mut img = vec![];
    let b = 0.5;
    for y in 0..h {
        for x in 0..w {
            if (x + y) % 2 == 0 {
                img.push(ColorRgb::new(b, b, b));
            } else {
                img.push(ColorRgb::new(1.0, 1.0, 1.0));
            }
        }
    }
    save_image(&img, (w, h), "checkerboard.png").unwrap();
}
