use super::*;

pub struct FrameState {
    prev: Instant,
    delta: Duration,
    buffer: VecDeque<Duration>,
    frame_index: u64,
    frame_count: u64,
    display_elapsed: Duration,
    display_delta: f64,
}

const BUFFER_SIZE: usize = 60;
const TRIGGER_TIME: f32 = 0.5;

impl FrameState {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            prev: now,
            delta: Duration::default(),
            buffer: VecDeque::with_capacity(BUFFER_SIZE),
            frame_index: 0,
            frame_count: 0,
            display_elapsed: Duration::default(),
            display_delta: 0.0,
        }
    }

    pub fn update(&mut self) {
        self.delta = self.prev.elapsed();
        self.prev = Instant::now();
        self.buffer.push_front(self.delta);
        if self.buffer.len() > BUFFER_SIZE {
            self.buffer.pop_back();
        }

        self.display_elapsed += self.delta;
        if self.display_elapsed.as_secs_f32() > TRIGGER_TIME {
            let sum: f64 = self.buffer.iter().map(Duration::as_secs_f64).sum();
            self.display_delta = sum / self.buffer.len() as f64;
            self.display_elapsed = Duration::default();
        }

        self.frame_count += 1;
        self.frame_index = self.frame_count % u64::from(vulkan::MAX_CONCURRENT_FRAMES);
    }

    pub fn delta(&self) -> Duration {
        self.delta
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }
}

impl std::fmt::Display for FrameState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let delta = self.display_delta;
        let delta_ms = 1e3 * delta;
        let fps = delta.recip();
        let frame = self.frame_count;
        write!(
            f,
            "FPS: {fps:.03}\nDelta: {delta_ms:.03} ms\nFrame: {frame}"
        )
    }
}
