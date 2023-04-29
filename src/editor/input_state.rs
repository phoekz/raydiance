pub struct InputState {
    pub a: bool,
    pub d: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self { a: false, d: false }
    }

    pub fn handle_event(&mut self, event: &winit::event::Event<()>) {
        let winit::event::Event::WindowEvent { event, .. } = event else {
            return;
        };

        let winit::event::WindowEvent::KeyboardInput { input, .. } = event else {
            return;
        };

        let pressed = match input.state {
            winit::event::ElementState::Pressed => true,
            winit::event::ElementState::Released => false,
        };

        let Some(virtual_keycode) = input.virtual_keycode else {
            return;
        };

        match virtual_keycode {
            winit::event::VirtualKeyCode::A => {
                self.a = pressed;
            }
            winit::event::VirtualKeyCode::D => {
                self.d = pressed;
            }
            _ => {}
        }
    }
}
