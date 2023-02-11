use super::*;

//
// Tweenable
//

pub trait Tweenable {
    fn tween(&self, rhs: &Self, time: f32) -> Self;
}

impl Tweenable for f32 {
    fn tween(&self, rhs: &Self, time: f32) -> Self {
        lerp_scalar(*self, *rhs, time)
    }
}

impl Tweenable for ColorRgb {
    fn tween(&self, rhs: &Self, time: f32) -> Self {
        lerp_color(self, rhs, time)
    }
}

impl Tweenable for glb::DynamicTexture {
    fn tween(&self, rhs: &Self, time: f32) -> Self {
        match (*self, *rhs) {
            (Self::Scalar(lhs), Self::Scalar(rhs)) => Self::Scalar(lerp_scalar(lhs, rhs, time)),
            (Self::Vector2(lhs), Self::Vector2(rhs)) => Self::Vector2(lerp_array(lhs, rhs, time)),
            (Self::Vector3(lhs), Self::Vector3(rhs)) => Self::Vector3(lerp_array(lhs, rhs, time)),
            (Self::Vector4(lhs), Self::Vector4(rhs)) => Self::Vector4(lerp_array(lhs, rhs, time)),
            _ => panic!("glb::DynamicTextures must have the same enum variant, got {self:?} and {rhs:?} instead"),
        }
    }
}

//
// Easing functions
//

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum EasingFunction {
    Linear,
    CubicInOut,
}

impl Default for EasingFunction {
    fn default() -> Self {
        Self::Linear
    }
}

impl EasingFunction {
    fn tween(self, time: f32) -> f32 {
        use easer::functions::*;
        assert!((0.0..=1.0).contains(&time));
        match self {
            EasingFunction::Linear => time,
            EasingFunction::CubicInOut => Cubic::ease_in_out(time, 0.0, 1.0, 1.0),
        }
    }
}

//
// Keyframe
//

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Keyframe<T: Tweenable>(f32, T, #[serde(default)] EasingFunction);

impl<T> Keyframe<T>
where
    T: Tweenable,
{
    pub fn new(time: f32, value: T, function: EasingFunction) -> Self {
        Self(time, value, function)
    }
}

//
// Sequence
//

#[derive(Debug)]
pub struct Sequence<T>
where
    T: Tweenable,
{
    time_bounds: (f32, f32),
    times: Vec<f32>,
    values: Vec<T>,
    functions: Vec<EasingFunction>,
}

impl<T> From<Vec<Keyframe<T>>> for Sequence<T>
where
    T: Tweenable,
{
    fn from(keyframes: Vec<Keyframe<T>>) -> Self {
        assert!(
            keyframes.len() > 1,
            "Sequence must contain 2 or more keyframes, got {} instead",
            keyframes.len()
        );

        let mut keyframes = keyframes;
        keyframes.sort_by(|ka, kb| ka.0.total_cmp(&kb.0));
        let mut times = Vec::with_capacity(keyframes.len());
        let mut values = Vec::with_capacity(keyframes.len());
        let mut functions = Vec::with_capacity(keyframes.len());
        for Keyframe(time, value, function) in keyframes {
            times.push(time);
            values.push(value);
            functions.push(function);
        }
        let time_bounds = (*times.first().unwrap(), *times.last().unwrap());
        Self {
            time_bounds,
            times,
            values,
            functions,
        }
    }
}

impl<T> Sequence<T>
where
    T: Tweenable + Clone + Copy,
{
    pub fn tween(&self, time: f32) -> T {
        if time <= self.time_bounds.0 {
            return *self.values.first().unwrap();
        }
        if time > self.time_bounds.1 {
            return *self.values.last().unwrap();
        }
        let i = self.times.partition_point(|kf| *kf < time);
        let i = i.min(self.times.len() - 1);
        let start_time = self.times[i - 1];
        let start_value = &self.values[i - 1];
        let func = self.functions[i - 1];
        let end_time = self.times[i];
        let end_value = &self.values[i];
        let time = (time - start_time) / (end_time - start_time);
        start_value.tween(end_value, func.tween(time))
    }

    pub fn max_time(&self) -> f32 {
        self.time_bounds.1
    }
}

//
// Values
//

pub enum Value<T>
where
    T: anim::Tweenable + Clone + Copy,
{
    Constant(T),
    Sequence(anim::Sequence<T>),
}

impl<T> From<cfg::Value<T>> for Value<T>
where
    T: vz::anim::Tweenable + Clone + Copy,
{
    fn from(value: cfg::Value<T>) -> Self {
        match value {
            cfg::Value::Constant(c) => Value::Constant(c),
            cfg::Value::Keyframes(kfs) => Value::Sequence(kfs.into()),
        }
    }
}

impl<T> Value<T>
where
    T: vz::anim::Tweenable + Clone + Copy,
{
    pub fn value(&self, time: f32) -> T {
        match self {
            Self::Constant(c) => *c,
            Self::Sequence(seq) => seq.tween(time),
        }
    }

    pub fn max_time(&self) -> f32 {
        match self {
            Value::Constant(_) => 1.0,
            Value::Sequence(seq) => seq.max_time(),
        }
    }
}

//
// Tests
//

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! kf {
        ($t:expr, $v:expr) => {
            Keyframe($t, $v, EasingFunction::default())
        };
    }

    macro_rules! kfc {
        ($t:expr, $v:expr) => {
            Keyframe($t, $v, EasingFunction::CubicInOut)
        };
    }

    #[test]
    fn simple() {
        let keyframes = vec![kf!(0.0, 0.0), kf!(1.0, 1.0), kf!(2.0, 2.0)];
        let sequence = Sequence::from(keyframes);
        assert_ulps_eq!(sequence.tween(0.0), 0.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(0.25), 0.25, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(0.5), 0.5, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(1.0), 1.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(1.5), 1.5, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(2.0), 2.0, max_ulps = 1);
        cli_plot(&sequence, 0.0, 2.0);
    }

    #[test]
    fn variable() {
        let keyframes = vec![kf!(0.0, 0.0), kf!(1.0, 1.0), kf!(10.0, 10.0)];
        let sequence = Sequence::from(keyframes);
        assert_ulps_eq!(sequence.tween(0.0), 0.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(1.0), 1.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(2.0), 2.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(5.0), 5.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(7.5), 7.5, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(10.0), 10.0, max_ulps = 1);
        cli_plot(&sequence, 0.0, 10.0);
    }

    #[test]
    fn unsorted() {
        let keyframes = vec![kf!(1.0, 1.0), kf!(0.0, 0.0)];
        let sequence = Sequence::from(keyframes);
        assert_ulps_eq!(sequence.tween(0.0), 0.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(1.0), 1.0, max_ulps = 1);
        cli_plot(&sequence, 0.0, 1.0);
    }

    #[test]
    fn uneven() {
        let keyframes = vec![kf!(0.0, 0.0), kf!(1.0, 1.0), kf!(2.0, 10.0)];
        let sequence = Sequence::from(keyframes);
        assert_ulps_eq!(sequence.tween(0.0), 0.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(0.5), 0.5, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(1.0), 1.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(1.5), 5.5, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(2.0), 10.0, max_ulps = 1);
        cli_plot(&sequence, 0.0, 2.0);
    }

    #[test]
    fn nonlinear() {
        let keyframes = vec![kfc!(0.0, 0.0), kfc!(1.0, 1.0), kfc!(2.0, 10.0)];
        let sequence = Sequence::from(keyframes);
        assert_ulps_eq!(sequence.tween(0.0), 0.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(2.0), 10.0, max_ulps = 1);
        assert!(
            sequence.tween(0.9) < sequence.tween(1.1),
            "Sequence must be monotonically increasing"
        );
        cli_plot(&sequence, 0.0, 2.0);
    }

    #[test]
    fn out_of_bounds() {
        let keyframes = vec![kf!(0.0, 0.0), kf!(1.0, 1.0)];
        let sequence = Sequence::from(keyframes);
        assert_ulps_eq!(sequence.tween(-1.0), 0.0, max_ulps = 1);
        assert_ulps_eq!(sequence.tween(2.0), 1.0, max_ulps = 1);
    }

    #[test]
    #[should_panic(expected = "Sequence must contain 2 or more keyframes, got 0 instead")]
    fn fail_empty_slice() {
        let keyframes: Vec<Keyframe<f32>> = vec![];
        let _sequence = Sequence::from(keyframes);
    }

    /// Usage: `cli_plot(&sequence, 0.0, 2.0);`
    #[allow(dead_code)]
    fn cli_plot(sequence: &Sequence<f32>, xmin: f32, xmax: f32) {
        use textplots::{Chart, Plot, Shape};

        const SAMPLES: i32 = 512;

        let mut data = vec![];
        for idx in 0..SAMPLES {
            let i = idx as f32 / SAMPLES as f32;
            let i = xmax * i;
            let v = sequence.tween(i);
            data.push((i, v));
        }

        let mut chart = Chart::new(120, 60, xmin, xmax);
        let shape = Shape::Points(&data);
        let plot = chart.lineplot(&shape);
        plot.nice();
    }
}
