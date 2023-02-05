use super::*;

pub struct Params {
    pub delay_num: u16,
    pub delay_den: u16,
}

pub fn render<Frames, ImagePath>(params: &Params, path: ImagePath, frames: Frames) -> Result<()>
where
    ImagePath: AsRef<Path>,
    Frames: IntoIterator<Item = image::Rgb>,
{
    let frame_images = frames.into_iter().collect::<Vec<_>>();
    let width = frame_images[0].width();
    let height = frame_images[0].height();
    let num_frames = frame_images.len() as u32;

    let mut writer = BufWriter::new(File::create(path.as_ref())?);
    let config = apng::Config {
        width,
        height,
        num_frames,
        num_plays: 0,
        color: png::ColorType::Rgb,
        depth: png::BitDepth::Eight,
        filter: png::FilterType::NoFilter,
    };
    let mut encoder = apng::Encoder::new(&mut writer, config)?;
    let frame = apng::Frame {
        delay_num: Some(params.delay_num),
        delay_den: Some(params.delay_den),
        ..apng::Frame::default()
    };
    let images = frame_images
        .into_iter()
        .map(|image| {
            apng::load_dynamic_image(image.into_dynamic()).expect("Failed to load image into apng")
        })
        .collect::<Vec<_>>();
    encoder.encode_all(images, Some(&frame))?;
    info!("Wrote to {}", path.as_ref().display());

    Ok(())
}

pub fn create_boomerang(frames: Vec<image::Rgb>) -> Vec<image::Rgb> {
    frames
        .clone()
        .into_iter()
        .chain(frames.into_iter().rev())
        .collect()
}
