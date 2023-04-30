use super::*;

//
// Modules
//

mod camera;
mod control_flow;
mod frame_state;
mod gui;
mod input_state;
mod material_editor;
mod window;

use camera::Camera;
use control_flow::ControlFlow;
use frame_state::FrameState;
use gui::Gui;
use input_state::InputState;
use material_editor::{MaterialEditor, MaterialEditorState};

//
// Re-exports.
//

pub(crate) use gui::GuiElement;
pub(crate) use window::{Window, WindowSize};

//
// Editor
//

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub glb_scene: PathBuf,
}

pub fn run(args: Args) -> Result<()> {
    use winit::platform::run_return::EventLoopExtRunReturn;

    // Init window.
    let window_aspect = DEFAULT_ASPECT_RATIO;
    let (window, mut event_loop) = Window::create(&window::Params {
        title: env!("CARGO_PKG_NAME"),
        size: WindowSize {
            w: window_aspect.0 * 50,
            h: window_aspect.1 * 50,
        },
        min_size: WindowSize {
            w: window_aspect.0 * 20,
            h: window_aspect.1 * 20,
        },
        decorations: true,
    })?;

    // Init editor.
    let mut editor = Editor::create(window, &args.glb_scene)?;

    // Event loop.
    event_loop.run_return(|event, _, control_flow| {
        // Control flow - event handler.
        *control_flow = ControlFlow::handle_event(&event);

        // Editor - event handler.
        if let Err(err) = editor.handle_event(&event) {
            error!("{err}");
            *control_flow = control_flow::EXIT;
        }
    });

    // Cleanup.
    editor.destroy()?;

    Ok(())
}

struct Editor {
    window: Window,
    gui: Gui,
    rds_scene: rds::Scene,
    dyn_scene: rds::DynamicScene,
    raytracer: cpupt::Raytracer,
    renderer: vulkan::Renderer,
    frame_state: FrameState,
    input_state: InputState,
    camera: Camera,

    any_window_focused: bool,
    latest_output: Option<cpupt::Output>,
    sampling_status: cpupt::SamplingStatus,
    image_name: String,

    display_raytracing_image: bool,
    hemisphere_sampler: cpupt::HemisphereSampler,
    material_editor_state: MaterialEditorState,
    visualize_normals: bool,
    tonemapping: bool,
    exposure: cpupt::Exposure,
    sky_params: cpupt::SkyParams,
}

impl Editor {
    fn create(window: Window, glb_scene: &Path) -> Result<Self> {
        let (rds_scene, dyn_scene) = rds::Scene::create(
            &std::fs::read(glb_scene)
                .with_context(|| format!("Reading glb scene: {}", glb_scene.display()))?,
        )?;
        let raytracer = cpupt::Raytracer::create(
            cpupt::Params {
                samples_per_pixel: 256,
                ..cpupt::Params::default()
            },
            rds_scene.clone(),
        );
        let mut gui = Gui::create(&window);
        let renderer = unsafe {
            vulkan::Renderer::create(
                &window,
                window.title(),
                window.size(),
                &rds_scene,
                &gui.font_atlas_texture(),
            )?
        };

        Ok(Self {
            window,
            gui,
            rds_scene,
            dyn_scene,
            raytracer,
            renderer,
            frame_state: FrameState::new(),
            input_state: InputState::new(),
            camera: Camera::new(),

            any_window_focused: false,
            latest_output: None,
            sampling_status: cpupt::SamplingStatus::new(),
            image_name: String::from("image"),

            display_raytracing_image: true,
            hemisphere_sampler: cpupt::HemisphereSampler::default(),
            material_editor_state: MaterialEditorState::new(),
            visualize_normals: false,
            tonemapping: true,
            exposure: cpupt::Exposure::default(),
            sky_params: cpupt::SkyParams::default(),
        })
    }

    fn destroy(self) -> Result<()> {
        self.raytracer.terminate()?;
        unsafe { self.renderer.destroy()? };
        Ok(())
    }

    fn handle_event(&mut self, event: &winit::event::Event<()>) -> Result<()> {
        // Window - event handler.
        self.window.handle_event(event);

        // Gui - event handler.
        self.gui.handle_event(&self.window, event);

        // Inputs - event handler.
        if !self.any_window_focused {
            self.input_state.handle_event(event);
        }

        match event {
            winit::event::Event::NewEvents(_) => {
                self.new_events();
            }
            winit::event::Event::MainEventsCleared => {
                self.main_events_cleared()?;
            }
            winit::event::Event::RedrawRequested(_) => {
                self.redraw_requested()?;
            }
            _ => {}
        }

        Ok(())
    }

    fn new_events(&mut self) {
        // Update frame state.
        self.frame_state.update();

        // Update gui.
        self.gui.update_delta_time(self.frame_state.delta());
    }

    fn main_events_cleared(&mut self) -> Result<()> {
        // Update gui.
        self.gui.prepare_frame(&self.window)?;

        // Update camera.
        self.camera.update(&self.input_state, &self.frame_state);

        // Update raytracer.
        self.raytracer.send_input(cpupt::Input {
            camera_transform: self.camera.transform(),
            image_size: self.window.size().into(),
            hemisphere_sampler: self.hemisphere_sampler,
            dyn_scene: self.dyn_scene.clone(),
            visualize_normals: self.visualize_normals,
            tonemapping: self.tonemapping,
            exposure: self.exposure,
            sky_params: self.sky_params,
            salt: None,
        })?;

        // Draw screen.
        self.window.handle().request_redraw();

        Ok(())
    }

    fn redraw_requested(&mut self) -> Result<()> {
        // Generate gui.
        self.gui.frame(&self.window, |ui| {
            // Update state.
            self.any_window_focused =
                ui.is_window_focused_with_flags(imgui::WindowFocusedFlags::ANY_WINDOW);

            // Main window.
            let window = ui
                .window("Raydiance")
                .size(
                    [220.0, self.window.size().h as f32],
                    imgui::Condition::FirstUseEver,
                )
                .position_pivot([0.0, 0.0])
                .position([0.0, 0.0], imgui::Condition::FirstUseEver)
                .collapsible(true)
                .resizable(false)
                .movable(false);
            window.build(|| {
                // Frame state.
                self.frame_state.gui(ui);

                // Sampling status.
                self.sampling_status.gui(ui);

                ui.separator();

                // Material editor.
                MaterialEditor::new(
                    &mut self.material_editor_state,
                    &mut self.rds_scene,
                    &mut self.dyn_scene,
                )
                .gui(ui);

                ui.separator();

                // Sky model.
                self.sky_params.gui(ui);

                ui.separator();

                // Rendering config.
                self.exposure.gui(ui);
                ui.checkbox("Visualize normals", &mut self.visualize_normals);
                ui.checkbox("Tonemapping", &mut self.tonemapping);
                self.hemisphere_sampler.gui(ui);

                ui.separator();

                // Image utilities.
                ui.checkbox("Show raytracing image", &mut self.display_raytracing_image);
                imgui::InputText::new(ui, "Image name", &mut self.image_name).build();
                if ui.button("Save image") {
                    if let Some(output) = &self.latest_output {
                        save_image_to_file(&self.image_name, output).expect("Saving image to file");
                    }
                }
            });
        });

        // Fast forward to the latest input, in case the raytracer sends
        // frames faster than the main loop.
        let mut output = None;
        while let Some(current_output) = self.raytracer.try_recv_output() {
            output = Some(current_output);
        }

        unsafe {
            // If we got a new output, submit it to the rasterizer.
            if let Some(output) = output {
                self.renderer
                    .update_raytracing_image(&output.image, output.image_size)?;
                self.sampling_status = output.sampling_status;
                self.latest_output = Some(output);
            }

            // Update gui.
            let gui_data = self.gui.render();
            self.renderer
                .update_gui(self.frame_state.frame_index(), gui_data)?;

            // Rasterize.
            self.renderer.redraw(
                &self.dyn_scene,
                self.window.size(),
                self.window.new_size(),
                self.frame_state.frame_index(),
                self.camera.transform(),
                self.display_raytracing_image,
                self.visualize_normals,
                gui_data,
            )?;
        }

        self.window.handled_resize();

        Ok(())
    }
}

fn save_image_to_file(image_name: &str, output: &cpupt::Output) -> Result<()> {
    let path = if image_name.is_empty() {
        let timestamp = utc_timestamp()?;
        PathBuf::from(format!("{timestamp}.png"))
    } else {
        PathBuf::from(format!("{image_name}.png"))
    };
    let image = vz::image::Rgb::from_colors(&output.image, output.image_size);
    image
        .save(&path)
        .with_context(|| format!("Failed to save image to {}", path.display()))?;
    info!(
        "Wrote {}x{} image to {}",
        output.image_size.0,
        output.image_size.1,
        path.display()
    );
    Ok(())
}
