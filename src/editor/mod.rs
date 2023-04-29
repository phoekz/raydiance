use super::*;

//
// Modules
//

mod control_flow;
mod frame_state;
mod gui;
mod inputs;
mod window;

use control_flow::ControlFlow;
use frame_state::FrameState;
use gui::Gui;
use inputs::Inputs;

//
// Re-exports.
//

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
    let window_aspect = (16, 9);
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

    inputs: Inputs,
    any_window_focused: bool,
    latest_output: Option<cpupt::Output>,
    sample_state: (u32, u32),
    image_name: String,

    camera_angle: f32,
    camera_transform: Mat4,
    display_raytracing_image: bool,
    hemisphere_sampler: HemisphereSampler,
    selected_material: usize,
    visualize_normals: bool,
    tonemapping: bool,
    exposure: Exposure,
    sky_params: cpupt::sky::ext::StateExtParams,
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
            inputs: Inputs::new(),
            latest_output: None,
            any_window_focused: false,
            sample_state: (0, 0),
            image_name: String::from("image"),

            camera_angle: 0.0,
            camera_transform: Mat4::identity(),
            display_raytracing_image: true,
            hemisphere_sampler: HemisphereSampler::default(),
            selected_material: 0,
            visualize_normals: false,
            tonemapping: true,
            exposure: Exposure::default(),
            sky_params: cpupt::sky::ext::StateExtParams::default(),
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
            self.inputs.handle_event(event);
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
        let speed = TAU / 5.0;
        let delta_time = self.frame_state.delta().as_secs_f32();
        if self.inputs.a {
            self.camera_angle -= speed * delta_time;
        }
        if self.inputs.d {
            self.camera_angle += speed * delta_time;
        }
        self.camera_transform = Mat4::from_axis_angle(&Vec3::y_axis(), self.camera_angle);

        // Update raytracer.
        self.raytracer.send_input(cpupt::Input {
            camera_transform: self.camera_transform,
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
                // Performance counters.
                {
                    ui.text(format!("{}", self.frame_state));
                }

                // Rendering status.
                {
                    let style = ui.clone_style();
                    let progress = self.sample_state.0 as f32 / self.sample_state.1 as f32;
                    imgui::ProgressBar::new(progress).size([0.0, 0.0]).build(ui);
                    ui.same_line();
                    ui.same_line_with_spacing(0.0, style.item_inner_spacing[0]);
                    ui.text("Rendering");
                }

                ui.separator();

                // Material editor.
                {
                    let rds_scene = &self.rds_scene;
                    let dyn_scene = &mut self.dyn_scene;

                    // Selector.
                    ui.combo(
                        "Material",
                        &mut self.selected_material,
                        &rds_scene.materials,
                        |material| Cow::Borrowed(&material.name),
                    );

                    let material = &mut dyn_scene.materials[self.selected_material];

                    // Material model.
                    {
                        let model = &mut material.model;
                        if let Some(token) = ui.begin_combo("Model", model.name()) {
                            if ui.selectable(rds::MaterialModel::Diffuse.name()) {
                                *model = rds::MaterialModel::Diffuse;
                            }
                            if ui.selectable(rds::MaterialModel::Disney.name()) {
                                *model = rds::MaterialModel::Disney;
                            }
                            token.end();
                        }
                    }

                    // Texture editor.
                    if let Some(_token) = ui.begin_table("", 3) {
                        ui.table_next_row();
                        ui.table_set_column_index(0);
                        {
                            let name = "Base color";
                            let _id = ui.push_id(name);
                            let index = material.base_color as usize;
                            let mut texture = &mut dyn_scene.textures[index];
                            let mut bit = dyn_scene.replaced_textures[index];

                            ui.text(name);
                            ui.table_next_column();

                            if let rds::DynamicTexture::Vector4(ref mut value) = &mut texture {
                                if ui
                                    .color_edit4_config("Value", value)
                                    .alpha(false)
                                    .inputs(false)
                                    .build()
                                {
                                    // Convenience: replace texture when an edit has been made without extra interaction.
                                    dyn_scene.replaced_textures.set(index, true);
                                }
                            }
                            ui.table_next_column();

                            {
                                if ui.checkbox("##use", &mut bit) {
                                    dyn_scene.replaced_textures.set(index, bit);
                                }
                                ui.same_line();
                                if ui.button("X") {
                                    // Convenience: reset to default value and clear replacement with one click.
                                    *texture = dyn_scene.default_textures[index];
                                    dyn_scene.replaced_textures.set(index, false);
                                }
                            }
                            ui.table_next_column();
                        }
                        {
                            let name = "Roughness";
                            let _id = ui.push_id(name);
                            let index = material.roughness as usize;
                            let mut texture = &mut dyn_scene.textures[index];
                            let mut bit = dyn_scene.replaced_textures[index];

                            ui.text(name);
                            ui.table_next_column();

                            if let rds::DynamicTexture::Scalar(ref mut value) = &mut texture {
                                if imgui::Drag::new("##slider")
                                    .range(0.0, 1.0)
                                    .speed(0.01)
                                    .build(ui, value)
                                {
                                    // Convenience: replace texture when an edit has been made without extra interaction.
                                    dyn_scene.replaced_textures.set(index, true);
                                }
                            }
                            ui.table_next_column();

                            {
                                if ui.checkbox("##use", &mut bit) {
                                    dyn_scene.replaced_textures.set(index, bit);
                                }
                                ui.same_line();
                                if ui.button("X") {
                                    // Convenience: reset to default value and clear replacement with one click.
                                    *texture = dyn_scene.default_textures[index];
                                    dyn_scene.replaced_textures.set(index, false);
                                }
                            }
                            ui.table_next_column();
                        }
                        {
                            let name = "Metallic";
                            let _id = ui.push_id(name);
                            let index = material.metallic as usize;
                            let mut texture = &mut dyn_scene.textures[index];
                            let mut bit = dyn_scene.replaced_textures[index];

                            ui.text(name);
                            ui.table_next_column();

                            if let rds::DynamicTexture::Scalar(ref mut value) = &mut texture {
                                if imgui::Drag::new("##slider")
                                    .range(0.0, 1.0)
                                    .speed(0.01)
                                    .build(ui, value)
                                {
                                    // Convenience: replace texture when an edit has been made without extra interaction.
                                    dyn_scene.replaced_textures.set(index, true);
                                }
                            }
                            ui.table_next_column();

                            {
                                if ui.checkbox("##use", &mut bit) {
                                    dyn_scene.replaced_textures.set(index, bit);
                                }
                                ui.same_line();
                                if ui.button("X") {
                                    // Convenience: reset to default value and clear replacement with one click.
                                    *texture = dyn_scene.default_textures[index];
                                    dyn_scene.replaced_textures.set(index, false);
                                }
                            }
                            ui.table_next_column();
                        }
                        {
                            let name = "Specular";
                            let _id = ui.push_id(name);
                            let index = material.specular as usize;
                            let mut texture = &mut dyn_scene.textures[index];
                            let mut bit = dyn_scene.replaced_textures[index];

                            ui.text(name);
                            ui.table_next_column();

                            if let rds::DynamicTexture::Scalar(ref mut value) = &mut texture {
                                if imgui::Drag::new("##slider")
                                    .range(0.0, 1.0)
                                    .speed(0.01)
                                    .build(ui, value)
                                {
                                    // Convenience: replace texture when an edit has been made without extra interaction.
                                    dyn_scene.replaced_textures.set(index, true);
                                }
                            }
                            ui.table_next_column();

                            {
                                if ui.checkbox("##use", &mut bit) {
                                    dyn_scene.replaced_textures.set(index, bit);
                                }
                                ui.same_line();
                                if ui.button("X") {
                                    // Convenience: reset to default value and clear replacement with one click.
                                    *texture = dyn_scene.default_textures[index];
                                    dyn_scene.replaced_textures.set(index, false);
                                }
                            }
                            ui.table_next_column();
                        }
                        {
                            let name = "Specular Tint";
                            let _id = ui.push_id(name);
                            let index = material.specular_tint as usize;
                            let mut texture = &mut dyn_scene.textures[index];
                            let mut bit = dyn_scene.replaced_textures[index];

                            ui.text(name);
                            ui.table_next_column();

                            if let rds::DynamicTexture::Scalar(ref mut value) = &mut texture {
                                if imgui::Drag::new("##slider")
                                    .range(0.0, 1.0)
                                    .speed(0.01)
                                    .build(ui, value)
                                {
                                    // Convenience: replace texture when an edit has been made without extra interaction.
                                    dyn_scene.replaced_textures.set(index, true);
                                }
                            }
                            ui.table_next_column();

                            {
                                if ui.checkbox("##use", &mut bit) {
                                    dyn_scene.replaced_textures.set(index, bit);
                                }
                                ui.same_line();
                                if ui.button("X") {
                                    // Convenience: reset to default value and clear replacement with one click.
                                    *texture = dyn_scene.default_textures[index];
                                    dyn_scene.replaced_textures.set(index, false);
                                }
                            }
                            ui.table_next_column();
                        }
                        {
                            let name = "Sheen";
                            let _id = ui.push_id(name);
                            let index = material.sheen as usize;
                            let mut texture = &mut dyn_scene.textures[index];
                            let mut bit = dyn_scene.replaced_textures[index];

                            ui.text(name);
                            ui.table_next_column();

                            if let rds::DynamicTexture::Scalar(ref mut value) = &mut texture {
                                if imgui::Drag::new("##slider")
                                    .range(0.0, 1.0)
                                    .speed(0.01)
                                    .build(ui, value)
                                {
                                    // Convenience: replace texture when an edit has been made without extra interaction.
                                    dyn_scene.replaced_textures.set(index, true);
                                }
                            }
                            ui.table_next_column();

                            {
                                if ui.checkbox("##use", &mut bit) {
                                    dyn_scene.replaced_textures.set(index, bit);
                                }
                                ui.same_line();
                                if ui.button("X") {
                                    // Convenience: reset to default value and clear replacement with one click.
                                    *texture = dyn_scene.default_textures[index];
                                    dyn_scene.replaced_textures.set(index, false);
                                }
                            }
                            ui.table_next_column();
                        }
                        {
                            let name = "Sheen Tint";
                            let _id = ui.push_id(name);
                            let index = material.sheen_tint as usize;
                            let mut texture = &mut dyn_scene.textures[index];
                            let mut bit = dyn_scene.replaced_textures[index];

                            ui.text(name);
                            ui.table_next_column();

                            if let rds::DynamicTexture::Scalar(ref mut value) = &mut texture {
                                if imgui::Drag::new("##slider")
                                    .range(0.0, 1.0)
                                    .speed(0.01)
                                    .build(ui, value)
                                {
                                    // Convenience: replace texture when an edit has been made without extra interaction.
                                    dyn_scene.replaced_textures.set(index, true);
                                }
                            }
                            ui.table_next_column();

                            {
                                if ui.checkbox("##use", &mut bit) {
                                    dyn_scene.replaced_textures.set(index, bit);
                                }
                                ui.same_line();
                                if ui.button("X") {
                                    // Convenience: reset to default value and clear replacement with one click.
                                    *texture = dyn_scene.default_textures[index];
                                    dyn_scene.replaced_textures.set(index, false);
                                }
                            }
                            ui.table_next_column();
                        }
                    }
                }

                ui.separator();

                // Sky model.
                {
                    let sky_params = &mut self.sky_params;
                    imgui::AngleSlider::new("Elevation")
                        .min_degrees(0.0)
                        .max_degrees(90.0)
                        .build(ui, &mut sky_params.elevation);
                    imgui::AngleSlider::new("Azimuth")
                        .min_degrees(0.0)
                        .max_degrees(360.0)
                        .build(ui, &mut sky_params.azimuth);
                    ui.slider("Turbidity", 1.0, 10.0, &mut sky_params.turbidity);
                    ui.color_edit3("Albedo", sky_params.albedo.as_mut());
                    self.exposure.gui(ui);
                }

                ui.separator();

                // Rendering config.
                ui.checkbox("Visualize normals", &mut self.visualize_normals);
                ui.checkbox("Tonemapping", &mut self.tonemapping);
                ui.text("Hemisphere sampler");
                if let Some(token) =
                    ui.begin_combo("##hemisphere_sampler", self.hemisphere_sampler.name())
                {
                    if ui.selectable(HemisphereSampler::Uniform.name()) {
                        self.hemisphere_sampler = HemisphereSampler::Uniform;
                    }

                    if ui.selectable(HemisphereSampler::Cosine.name()) {
                        self.hemisphere_sampler = HemisphereSampler::Cosine;
                    }

                    token.end();
                }

                // Image utilities.
                ui.checkbox("Show raytracing image", &mut self.display_raytracing_image);
                imgui::InputText::new(ui, "Image name", &mut self.image_name).build();
                if ui.button("Save image") {
                    if let Some(output) = &self.latest_output {
                        let timestamp = utc_timestamp().expect("Failed to generate timestamp");
                        let path = if self.image_name.is_empty() {
                            PathBuf::from(format!("{timestamp}.png"))
                        } else {
                            PathBuf::from(format!("{}.png", self.image_name))
                        };
                        let image = vz::image::Rgb::from_colors(&output.image, output.image_size);
                        image.save(&path).unwrap_or_else(|_| {
                            panic!("Failed to save image to {}", path.display())
                        });
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
                self.sample_state = (output.sample_index, output.sample_count);
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
                self.camera_transform,
                self.display_raytracing_image,
                self.visualize_normals,
                gui_data,
            )?;
        }

        self.window.handled_resize();

        Ok(())
    }
}
