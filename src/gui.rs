use super::*;

use imgui_winit_support::{HiDpiMode, WinitPlatform};

pub struct Gui {
    imgui: imgui::Context,
    winit: WinitPlatform,
}

impl Gui {
    pub fn create(window: &Window) -> Self {
        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);
        let mut winit = WinitPlatform::init(&mut imgui);
        winit.attach_window(imgui.io_mut(), window, HiDpiMode::Rounded);
        let font = include_bytes!("assets/fonts/SourceSansPro-Regular.ttf");
        let font_source = imgui::FontSource::TtfData {
            data: font,
            size_pixels: 14.0,
            config: None,
        };
        imgui.fonts().add_font(&[font_source]);
        imgui.io_mut().font_global_scale = (1.0 / winit.hidpi_factor()) as f32;
        imgui
            .io_mut()
            .backend_flags
            .insert(imgui::BackendFlags::RENDERER_HAS_VTX_OFFSET);
        Self { imgui, winit }
    }

    pub fn font_atlas_texture(&mut self) -> imgui::FontAtlasTexture {
        let fonts = self.imgui.fonts();
        fonts.tex_id = imgui::TextureId::new(0);
        let atlas_texture = fonts.build_rgba32_texture();
        atlas_texture
    }

    pub fn handle_event(&mut self, window: &Window, event: &Event<()>) {
        let io = self.imgui.io_mut();
        self.winit.handle_event(io, window, event);
    }

    pub fn update_delta_time(&mut self, delta: Duration) {
        let io = self.imgui.io_mut();
        io.update_delta_time(delta);
    }

    pub fn prepare_frame(&mut self, window: &Window) {
        let io = self.imgui.io_mut();
        self.winit.prepare_frame(io, window).unwrap();
    }

    pub fn frame<F>(&mut self, window: &Window, f: F)
    where
        F: FnOnce(&mut imgui::Ui),
    {
        let ui = self.imgui.frame();
        f(ui);
        self.winit.prepare_render(ui, window);
    }

    pub fn render(&mut self) -> &imgui::DrawData {
        self.imgui.render()
    }
}
