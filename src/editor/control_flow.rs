pub struct ControlFlow;

pub const CONTINUE: winit::event_loop::ControlFlow = winit::event_loop::ControlFlow::Poll;
pub const EXIT: winit::event_loop::ControlFlow = winit::event_loop::ControlFlow::Exit;

impl ControlFlow {
    pub fn handle_event(event: &winit::event::Event<()>) -> winit::event_loop::ControlFlow {
        let winit::event::Event::WindowEvent { event, .. } = event else {
            return CONTINUE;
        };

        if let winit::event::WindowEvent::CloseRequested = event {
            return EXIT;
        };

        let winit::event::WindowEvent::KeyboardInput { input, .. } = event else {
            return CONTINUE;
        };

        let Some(virtual_keycode) = input.virtual_keycode else {
            return CONTINUE;
        };

        if virtual_keycode == winit::event::VirtualKeyCode::Escape {
            return EXIT;
        };

        CONTINUE
    }
}
