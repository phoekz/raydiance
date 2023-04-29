use super::*;

pub struct Camera {
    angle: f32,
    transform: Mat4,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            angle: 0.0,
            transform: Mat4::identity(),
        }
    }

    pub fn update(&mut self, input: &InputState, frame: &FrameState) {
        let speed = TAU / 5.0;
        let delta_time = frame.delta().as_secs_f32();
        if input.a {
            self.angle -= speed * delta_time;
        }
        if input.d {
            self.angle += speed * delta_time;
        }
        self.transform = Mat4::from_axis_angle(&Vec3::y_axis(), self.angle);
    }

    pub fn transform(&self) -> Mat4 {
        self.transform
    }
}
