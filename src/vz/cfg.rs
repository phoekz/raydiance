use super::*;

// Note: `ConfigValue` is intended to be read and written by `serde`.
// `AnimationValue` is the actual value used in runtime.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Value<T>
where
    T: anim::Tweenable + Clone + Copy,
{
    Constant(T),
    Keyframes(Vec<anim::Keyframe<T>>),
}

macro_rules! keyframe {
    ($time:expr, $value:expr, $easing:ident) => {
        vz::anim::Keyframe::new($time, $value, vz::anim::EasingFunction::$easing)
    };
}
pub(crate) use keyframe;

pub fn read_from_file<P, T>(path: P) -> Result<T>
where
    P: AsRef<Path>,
    T: DeserializeOwned,
{
    from_reader(BufReader::new(File::open(path)?))
}

pub fn from_reader<R, T>(reader: R) -> Result<T>
where
    R: std::io::Read,
    T: DeserializeOwned,
{
    Ok(ron::de::from_reader(reader)?)
}

pub fn write_to_file<P, T>(path: P, value: &T) -> Result<()>
where
    P: AsRef<Path>,
    T: Serialize,
{
    to_writer(BufWriter::new(File::create(path)?), value)
}

pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
where
    W: std::io::Write,
    T: Serialize,
{
    let ron_config = ron::ser::PrettyConfig::default();
    ron::ser::to_writer_pretty(writer, &value, ron_config)?;
    Ok(())
}
