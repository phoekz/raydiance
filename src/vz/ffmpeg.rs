use super::*;

//
// Runner
//

#[derive(clap::Args)]
pub struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Transcodes a video into publishable videos.
    Transcode(TranscodeArgs),
    /// Stacks a set of videos into two columns and outputs a lossless video.
    Stack(StackArgs),
}

pub fn run(args: Args) -> Result<()> {
    // Validation.
    ffmpeg_exists()?;
    ffmpeg_check_formats()?;
    ffmpeg_check_codecs()?;
    ffmpeg_check_pixel_formats()?;

    // Execute command.
    match args.command {
        Commands::Transcode(args) => args.run()?,
        Commands::Stack(args) => args.run()?,
    }

    Ok(())
}

fn ffmpeg_exists() -> Result<()> {
    use which::which;
    let ffmpeg_path = which("ffmpeg").context("Looking for ffmpeg")?;
    info!("Found ffmpeg: {}", ffmpeg_path.display());
    Ok(())
}

fn ffmpeg_run<I, S>(args: I) -> Result<(String, String)>
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    use std::process::Command;
    let args = args.into_iter().map(|s| s.to_string()).collect::<Vec<_>>();
    debug!("Running ffmpeg with args: {args:#?}");
    let output = Command::new("ffmpeg").args(&args).output()?;
    let stdout = std::str::from_utf8(output.stdout.as_slice())?;
    let stderr = std::str::from_utf8(output.stderr.as_slice())?;
    ensure!(
        output.status.success(),
        "ffmpeg failed with stderr:\n{stderr}"
    );
    Ok((stdout.to_owned(), stderr.to_owned()))
}

fn ffmpeg_check_formats() -> Result<()> {
    let (formats, _) = ffmpeg_run(["-formats"])?;
    let mut found_apng = false;
    let mut found_webm = false;
    let mut found_mp4 = false;
    let mut found_mkv = false;
    for line in formats.lines() {
        if line.contains("apng") && line.contains("Animated Portable Network Graphics") {
            found_apng = true;
        }
        if line.contains("webm") && line.contains("WebM") {
            found_webm = true;
        }
        if line.contains("mp4") && line.contains("MP4") {
            found_mp4 = true;
        }
        if line.contains("matroska") && line.contains("Matroska") {
            found_mkv = true;
        }
    }
    ensure!(found_apng, "apng format must be supported");
    ensure!(found_webm, "webm format must be supported");
    ensure!(found_mp4, "mp4 format must be supported");
    ensure!(found_mkv, "mkv format must be supported");
    Ok(())
}

fn ffmpeg_check_codecs() -> Result<()> {
    let (codecs, _) = ffmpeg_run(["-codecs"])?;
    let mut found_apng = false;
    let mut found_h264 = false;
    let mut found_hevc = false;
    let mut found_vp9 = false;
    for line in codecs.lines() {
        if !line.trim().starts_with("DEV") {
            // Skip:
            // - Codecs with no decoding support.
            // - Codecs with no encoding support.
            // - Non-video codecs.
            continue;
        }

        if line.contains("apng") && line.contains("Animated Portable Network Graphics") {
            found_apng = true;
        }
        if line.contains("h264") && line.contains("H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10") {
            found_h264 = true;
        }
        if line.contains("hevc") && line.contains("H.265 / HEVC (High Efficiency Video Coding)") {
            found_hevc = true;
        }
        if line.contains("vp9") && line.contains("Google VP9") {
            found_vp9 = true;
        }
    }
    ensure!(found_apng, "apng codec must be supported");
    ensure!(found_h264, "h264 codec must be supported");
    ensure!(found_hevc, "hevc codec must be supported");
    ensure!(found_vp9, "vp9 codec must be supported");
    Ok(())
}

fn ffmpeg_check_pixel_formats() -> Result<()> {
    let (pix_fmts, _) = ffmpeg_run(["-pix_fmts"])?;
    let mut found_yuv420p = false;
    for line in pix_fmts.lines() {
        if line.starts_with("IO") && line.contains("yuv420p") {
            found_yuv420p = true;
        }
    }
    ensure!(found_yuv420p, "yuv420p pixel format must be supported");
    Ok(())
}

fn ffmpeg_encode_h265(input: &str, output: &str) -> Result<f64> {
    let time = Instant::now();
    ffmpeg_run([
        "-y",           // Overwrite output files without asking.
        "-hide_banner", // Suppress printing banner.
        "-i",           // Input file.
        input,          // .
        "-codec:v",     // Encoder.
        "libx265",      // .
        "-pix_fmt",     // Pixel format.
        "yuv420p",      // .
        "-crf",         // Constant rate factor.
        "16",           // .
        "-preset",      // Compression efficiency.
        "slow",         // .
        "-tag:v",       // Apple compatibility.
        "hvc1",         // .
        "-movflags",    // Move index (moov atom) to the beginning of the file.
        "faststart",    // .
        "-an",          // Disables audio recording.
        output,         // Output file.
    ])?;
    Ok(time.elapsed().as_secs_f64())
}

fn ffmpeg_encode_vp9(input: &str, output: &str) -> Result<f64> {
    let time = Instant::now();
    ffmpeg_run([
        "-y",           // Overwrite output files without asking.
        "-hide_banner", // Suppress printing banner.
        "-i",           // Input file.
        input,          // .
        "-codec:v",     // Encoder.
        "libvpx-vp9",   // .
        "-pix_fmt",     // Pixel format.
        "yuv420p",      // .
        "-crf",         // Constant rate factor.
        "18",           // .
        "-row-mt",      // VP9: Row based multi-threading.
        "1",            // .
        "-quality",     // VP9: Quality.
        "good",         // .
        "-speed",       // VP9: Higher = faster, but lower quality.
        "0",            // .
        "-an",          // Disables audio recording.
        output,         // Output file.
    ])?;
    Ok(time.elapsed().as_secs_f64())
}

//
// Transcode
//

#[derive(clap::Args)]
struct TranscodeArgs {
    #[arg(long)]
    input_file: PathBuf,

    #[arg(long)]
    output_directory: PathBuf,
}

impl TranscodeArgs {
    fn run(self) -> Result<()> {
        // Validation.
        ensure!(self.input_file.is_file(), "--input-file must be a file");
        ensure!(
            self.output_directory.is_dir(),
            "--output-directory must be a directory"
        );

        // Canonicalize paths.
        let input_file = self.input_file.to_string_lossy().replace('\\', "/");
        ensure!(input_file.ends_with(".apng") || input_file.ends_with(".mp4"));
        let file_name = self.input_file.file_stem().unwrap().to_string_lossy();
        let output_h265 = self
            .output_directory
            .join(format!("{file_name}-h265.mp4"))
            .to_string_lossy()
            .replace('\\', "/");
        let output_vp9 = self
            .output_directory
            .join(format!("{file_name}-vp9.webm"))
            .to_string_lossy()
            .replace('\\', "/");

        // Encoding.
        let time_h265 = ffmpeg_encode_h265(&input_file, &output_h265)?;
        let time_vp9 = ffmpeg_encode_vp9(&input_file, &output_vp9)?;

        // Report size.
        let bytes_input = file_size_fmt(&input_file);
        let bytes_h265 = file_size_fmt(&output_h265);
        let bytes_vp9 = file_size_fmt(&output_vp9);
        info!("encoding_h265={time_h265:.02} s, encoding_vp9={time_vp9:.02} s");
        info!("input={bytes_input} bytes, h265={bytes_h265} bytes, vp9={bytes_vp9} bytes");

        Ok(())
    }
}

//
// Stack
//

#[derive(clap::Args)]
struct StackArgs {
    #[arg(long)]
    output_file: PathBuf,

    input_files: Vec<PathBuf>,
}

impl StackArgs {
    fn run(self) -> Result<()> {
        // Validation.
        ensure!(self.input_files.len() >= 3);
        ensure!(self.input_files.iter().all(|f| f.is_file()));

        // Build complex filter string.
        let filter_parts = {
            // First, this routine stacks pairs of videos horizontally. The last
            // video without its pair is padded twice its length to match the
            // stacks above it. Then the horizontal stacks are stacked
            // vertically. Finally, the entire video is rounded down to an even
            // width and height.

            // An example output for a five video stack:
            //
            // [0:v][1:v]hstack[h0],
            // [2:v][3:v]hstack[h1],
            // [4:v]pad=w=2*iw:x=iw/2[h2],
            // [h0][h1]vstack[h],
            // [h][h2]vstack[h],
            // [h]crop=trunc(iw/2)*2:trunc(ih/2)*2

            let mut parts = vec![];

            // Build hstacks.
            let mut input_count = self.input_files.len();
            let mut input_index = 0;
            let mut hstack_index = 0;
            while input_count > 1 {
                parts.push(format!(
                    "[{}:v][{}:v]hstack[h{}]",
                    input_index,
                    input_index + 1,
                    hstack_index
                ));
                input_count -= 2;
                input_index += 2;
                hstack_index += 1;
            }
            // Special: the last odd video center-padded.
            if input_count > 0 {
                parts.push(format!(
                    "[{input_index}:v]pad=w=2*iw:x=iw/2[h{hstack_index}]",
                ));
                hstack_index += 1;
            }

            // Build vstacks.
            let mut vstack_index = 0;
            // Special: the first vstack is always between h0 and h1.
            parts.push("[h0][h1]vstack[v]".to_owned());
            vstack_index += 2;
            // vstacks afterwards always append to the `v`.
            while vstack_index < hstack_index {
                parts.push(format!("[v][h{vstack_index}]vstack[v]"));
                vstack_index += 1;
            }

            // Final rounding to avoid odd sizes.
            parts.push("[v]crop=trunc(iw/2)*2:trunc(ih/2)*2".to_owned());

            parts
        };

        // Join filter parts with commas.
        let mut filter = String::new();
        for (index, part) in filter_parts.into_iter().enumerate() {
            if index > 0 {
                filter.push(',');
            }
            filter.push_str(&part);
        }

        // Encoding.
        let mut args: Vec<Cow<str>> = vec![];
        args.push("-y".into());
        args.push("-hide_banner".into());
        for input in self.input_files {
            args.push("-i".into());
            args.push(input.to_string_lossy().replace('\\', "/").into());
        }
        args.push("-filter_complex".into());
        args.push(filter.into());
        args.push("-codec:v".into());
        args.push("libx264".into());
        args.push("-qp".into());
        args.push("0".into());
        args.push(self.output_file.to_string_lossy().replace('\\', "/").into());

        ffmpeg_run(args)?;

        Ok(())
    }
}

//
// Utils
//

fn file_size_fmt(path: &str) -> String {
    use num_format::{Locale, ToFormattedString};
    file_size(path).to_formatted_string(&Locale::en)
}

fn file_size(path: &str) -> u64 {
    std::fs::metadata(path).unwrap().len()
}
