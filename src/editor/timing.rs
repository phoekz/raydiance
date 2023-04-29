use super::*;

pub struct Timing {
    buffer: VecDeque<f32>,
    current_time: f32,
    display_avg_fps: f32,
    display_avg_time: f32,
}

const MAX_SIZE: usize = 8;
const TRIGGER_TIME: f32 = 0.125;

impl Timing {
    pub fn new() -> Timing {
        Self {
            buffer: VecDeque::with_capacity(MAX_SIZE),
            current_time: 0.0,
            display_avg_fps: 0.0,
            display_avg_time: 0.0,
        }
    }

    pub fn push(&mut self, delta_time: f32) {
        // Update ring buffer.
        self.buffer.push_front(delta_time);
        if self.buffer.len() > MAX_SIZE {
            self.buffer.pop_back();
        }

        // Update (slowed down) display times.
        self.current_time += delta_time;
        if self.current_time > TRIGGER_TIME {
            self.display_avg_time = self.avg_time();
            self.display_avg_fps = self.display_avg_time.recip();
            self.current_time = 0.0;
        }
    }

    fn avg_time(&self) -> f32 {
        let mut sum = 0.0;
        for &delta_time in &self.buffer {
            sum += delta_time;
        }
        sum / self.buffer.len() as f32
    }

    pub fn display_text(&self) -> String {
        format!(
            "FPS: {:.03}, Delta: {:.03} ms",
            self.display_avg_fps,
            1e3 * self.display_avg_time
        )
    }
}
