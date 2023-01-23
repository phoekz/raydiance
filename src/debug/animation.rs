use super::*;

pub fn create_boomerang(frames: Vec<img::Frame>) -> Vec<img::Frame> {
    frames
        .clone()
        .into_iter()
        .chain(frames.into_iter().rev())
        .collect()
}

pub fn render<Frames, ImagePath>(path: ImagePath, frames: Frames) -> Result<()>
where
    ImagePath: AsRef<Path>,
    Frames: IntoIterator<Item = img::Frame>,
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
        delay_num: Some(ANIMATION_DELAY_NUM),
        delay_den: Some(ANIMATION_DELAY_DEN),
        ..apng::Frame::default()
    };
    let images = frame_images
        .into_iter()
        .map(|image| {
            let image: image::DynamicImage = image.into();
            apng::load_dynamic_image(image).expect("Failed to load image into apng")
        })
        .collect::<Vec<_>>();
    encoder.encode_all(images, Some(&frame))?;
    info!("Wrote to {}", path.as_ref().display());

    Ok(())
}
