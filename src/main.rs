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
    clippy::wildcard_imports
)]

use std::{
    borrow::Cow,
    collections::VecDeque,
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
use clap::{Parser, Subcommand};
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

mod cpupt;
mod glb;
mod gui;
mod vulkan;
mod window;

use cpupt::{
    bsdf::DiffuseModel,
    sampling::{HemisphereSampler, OrthonormalBasis, UniformSampler},
};

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
    DebugImportanceSampling,
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
        Commands::DebugImportanceSampling => debug_importance_sampling(),
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
    let glb_scene = glb::Scene::create(include_bytes!("assets/rounded_cube.glb"))?;

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
    let mut diffuse_model = DiffuseModel::default();
    let mut latest_output: Option<cpupt::Output> = None;
    let mut sample_state = (0, 0);
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
                    diffuse_model,
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

                            ui.text(delta_times.display_text());

                            {
                                let progress = sample_state.0 as f32 / sample_state.1 as f32;
                                imgui::ProgressBar::new(progress).size([0.0, 0.0]).build(ui);
                                ui.same_line();
                                ui.same_line_with_spacing(0.0, style.item_inner_spacing[0]);
                                ui.text("Rendering");
                            }

                            if let Some(token) =
                                ui.begin_combo("Hemisphere sampler", hemisphere_sampler.name())
                            {
                                if ui.selectable(HemisphereSampler::Uniform.name()) {
                                    hemisphere_sampler = HemisphereSampler::Uniform;
                                };

                                if ui.selectable(HemisphereSampler::Cosine.name()) {
                                    hemisphere_sampler = HemisphereSampler::Cosine;
                                };

                                token.end();
                            }

                            if let Some(token) =
                                ui.begin_combo("Diffuse model", diffuse_model.name())
                            {
                                if ui.selectable(DiffuseModel::Lambert.name()) {
                                    diffuse_model = DiffuseModel::Lambert;
                                };

                                if ui.selectable(DiffuseModel::Disney.name()) {
                                    diffuse_model = DiffuseModel::Disney;
                                };

                                token.end();
                            }

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
                                    info!("{timestamp}.png");
                                    let path = format!("{timestamp}.png");
                                    save_image(&output.image, output.image_size, &path)
                                        .expect(&format!("Failed to save image to {path}"))
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
                            window_size,
                            resized_window_size,
                            frame_index,
                            camera_transform,
                            display_raytracing_image,
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
// Debug: importance sampling
//

fn debug_importance_sampling() -> Result<()> {
    enum SampleSequence {
        Grid(u32),
        Random(UniformSampler),
        Sobol,
    }

    impl SampleSequence {
        fn name(&self) -> &'static str {
            match self {
                SampleSequence::Grid(_) => "grid",
                SampleSequence::Random(_) => "random",
                SampleSequence::Sobol => "sobol",
            }
        }

        fn sample(&mut self, sample_index: u32) -> (f32, f32) {
            match self {
                SampleSequence::Grid(size) => {
                    let x = sample_index % *size;
                    let y = sample_index / *size;
                    (
                        (x as f32 + 0.5) / (*size as f32),
                        (y as f32 + 0.5) / (*size as f32),
                    )
                }
                SampleSequence::Random(uniform) => (uniform.sample(), uniform.sample()),
                SampleSequence::Sobol => (
                    sobol_burley::sample(sample_index, 0, 0),
                    sobol_burley::sample(sample_index, 1, 0),
                ),
            }
        }
    }

    let sample_grid_size = 16;
    let sample_count = sample_grid_size * sample_grid_size;

    let mut image_side = image::RgbaImage::new(100, 25);
    for py in 0..image_side.height() {
        for px in 0..image_side.width() {
            let nx = (px as f32 + 0.5) / image_side.width() as f32;
            let ny = (py as f32 + 0.5) / image_side.height() as f32;
            let ny = 1.0 - ny;
            let theta = TAU * nx;
            let phi = 0.5 * PI * ny;
            let x = theta.cos() * phi.sin();
            let y = phi.cos();
            let z = theta.sin() * phi.sin();
            let v = na::Vector3::new(x, y, z).normalize();
            let n = na::Vector3::y_axis();
            let cos_theta = v.dot(&n);
            assert!(cos_theta >= 0.0 && cos_theta <= 1.0);
            let c = (255.0 * cos_theta) as u8;
            image_side.put_pixel(px, py, image::Rgba([c, c, c, 255]));
        }
    }

    let mut image_top = image::RgbaImage::new(64, 64);
    for py in 0..image_top.height() {
        for px in 0..image_top.width() {
            let x = (px as f32 + 0.5) / image_top.width() as f32;
            let x = 2.0 * (x - 0.5);
            let z = (py as f32 + 0.5) / image_top.height() as f32;
            let z = 2.0 * (z - 0.5);
            let y = na::vector![x, z].norm();
            if y > 1.0 {
                image_top.put_pixel(px, py, image::Rgba([0, 0, 0, 255]));
                continue;
            }
            let y = 1.0 - y;
            assert!(x >= -1.0 && x <= 1.0, "px={px}, py={py}, x={x}");
            assert!(y >= -1.0 && y <= 1.0, "px={px}, py={py}, y={y}");
            assert!(z >= -1.0 && z <= 1.0, "px={px}, py={py}, z={z}");
            let v = na::vector![x, y, z];
            let n = na::Vector3::y_axis();
            let cos_theta = v.dot(&n);
            assert!(
                cos_theta >= 0.0 && cos_theta <= 1.0,
                "px={px}, py={py}, x={x}, y={y}, z={z} cos_theta={cos_theta}"
            );
            let c = (255.0 * cos_theta) as u8;
            image_top.put_pixel(px, py, image::Rgba([c, c, c, 255]));
        }
    }

    let sample_into_image_side = |image: image::RgbaImage,
                                  hemisphere: HemisphereSampler,
                                  sample_count: u32,
                                  sequence: &mut SampleSequence|
     -> Result<()> {
        let mut image = image;
        let width = image.width();
        let height = image.height();
        let onb = OrthonormalBasis::new(&na::Vector3::y_axis());
        for sample_index in 0..sample_count {
            let (e1, e2) = sequence.sample(sample_index);
            let sample = hemisphere.sample(&onb, e1, e2).into_inner();
            let x = sample.x;
            let y = sample.y;
            let z = sample.z;
            let theta = f32::atan2(z, x) + PI;
            let phi = f32::atan2(f32::sqrt(x * x + z * z), y);
            assert!(theta >= 0.0 && theta <= TAU);
            assert!(phi >= 0.0 && phi <= 0.5 * PI);
            let nx = theta / TAU;
            let ny = 1.0 - phi / (0.5 * PI);
            let px = ((nx * width as f32) as u32).min(width - 1);
            let py = ((ny * height as f32) as u32).min(height - 1);
            image.put_pixel(px, py, image::Rgba([255, 96, 0, 255]));
        }
        let image: image::DynamicImage = image.into();
        let image = image.flipv();
        let image = image.resize_exact(4 * width, 4 * height, image::imageops::Nearest);
        let strategy = sequence.name();
        let hemisphere = hemisphere.name().to_lowercase();
        let path = format!("work/side-{hemisphere}-{strategy}-{sample_count}.png");
        info!("Wrote {width}x{height} image to {path}");
        Ok(image.save(path)?)
    };

    let sample_into_image_top = |image: image::RgbaImage,
                                 hemisphere: HemisphereSampler,
                                 sample_count: u32,
                                 sequence: &mut SampleSequence|
     -> Result<()> {
        let mut image = image;
        let width = image.width();
        let height = image.height();
        let onb = OrthonormalBasis::new(&na::Vector3::y_axis());
        for sample_index in 0..sample_count {
            let (e1, e2) = sequence.sample(sample_index);
            let sample = hemisphere.sample(&onb, e1, e2).into_inner();
            let x = sample.dot(&na::Vector3::x_axis());
            let z = sample.dot(&na::Vector3::z_axis());
            assert!(x >= -1.0 && x <= 1.0, "e1={e1}, e2={e2}, x={x}");
            assert!(z >= -1.0 && z <= 1.0, "e1={e1}, e2={e2}, z={z}");
            let px = (((0.5 * (x + 1.0)) * width as f32) as u32).min(width - 1);
            let py = (((0.5 * (z + 1.0)) * height as f32) as u32).min(height - 1);
            image.put_pixel(px, py, image::Rgba([255, 96, 0, 255]));
        }
        let image: image::DynamicImage = image.into();
        let image = image.resize_exact(4 * width, 4 * height, image::imageops::Nearest);
        let strategy = sequence.name();
        let hemisphere = hemisphere.name().to_lowercase();
        let path = format!("work/top-{hemisphere}-{strategy}-{sample_count}.png");
        info!("Wrote {width}x{height} image to {path}");
        Ok(image.save(path)?)
    };

    for sequence in &mut [
        SampleSequence::Grid(sample_grid_size),
        SampleSequence::Random(UniformSampler::new()),
        SampleSequence::Sobol,
    ] {
        for &hemisphere in &[HemisphereSampler::Uniform, HemisphereSampler::Cosine] {
            sample_into_image_side(image_side.clone(), hemisphere, sample_count, sequence)?;
            sample_into_image_top(image_top.clone(), hemisphere, sample_count, sequence)?;
        }
    }

    Ok(())
}

//
// Misc
//

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
                img.push(LinSrgb::new(b, b, b));
            } else {
                img.push(LinSrgb::new(1.0, 1.0, 1.0));
            }
        }
    }
    save_image(&img, (w, h), "checkerboard.png").unwrap();
}
