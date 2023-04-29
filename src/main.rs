#![deny(future_incompatible)]
#![deny(nonstandard_style)]
#![deny(clippy::pedantic)]
#![allow(
    clippy::case_sensitive_file_extension_comparisons,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::collapsible_if,
    clippy::let_underscore_untyped,
    clippy::many_single_char_names,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::wildcard_imports
)]

use std::{
    borrow::Cow,
    collections::{HashSet, VecDeque},
    ffi::{CStr, CString},
    fs::File,
    io::{BufReader, BufWriter},
    mem::{size_of, transmute},
    num::{NonZeroU16, NonZeroU32},
    ops::Deref,
    path::{Path, PathBuf},
    slice,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, ensure, Context, Result};
use ash::vk;
use bitvec::prelude::*;
use bytemuck::{Pod, Zeroable};
use log::{debug, error, info, log, warn};
use nalgebra as na;
use rand::prelude::*;
use rayon::prelude::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[cfg(test)]
use approx::{assert_abs_diff_eq, assert_ulps_eq};

mod blog;
mod color;
mod cpupt;
mod debug;
mod editor;
mod gltf;
mod math;
mod offline;
mod rds;
mod vulkan;
mod vz;

use color::*;
use editor::GuiElement;
use math::*;

const PI: f32 = std::f32::consts::PI;
const TAU: f32 = std::f32::consts::TAU;
const INV_PI: f32 = std::f32::consts::FRAC_1_PI;

//
// Main
//

#[derive(clap::Parser)]
#[clap(author, version)]
struct CliArgs {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    Editor(editor::Args),
    Offline(offline::Args),
    Debug,
    BlogNew(blog::NewArgs),
    BlogBuild,
    BlogPlot,
    Ffmpeg(vz::ffmpeg::Args),
}

fn main() -> Result<()> {
    use clap::Parser;

    // Init logging.
    env_logger::init();

    // Execute command.
    match CliArgs::parse().command {
        Commands::Editor(args) => editor::run(args),
        Commands::Offline(args) => offline::run(args),
        Commands::Debug => debug::run(),
        Commands::BlogNew(args) => blog::new(args),
        Commands::BlogBuild => blog::build(),
        Commands::BlogPlot => blog::plot(),
        Commands::Ffmpeg(args) => vz::ffmpeg::run(args),
    }
}

//
// Utils
//

#[must_use]
pub fn manifest_dir() -> PathBuf {
    std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is not set")
        .into()
}

#[must_use]
pub fn work_dir() -> PathBuf {
    let work_dir = manifest_dir().join("work");
    if !work_dir.exists() {
        std::fs::create_dir(&work_dir).expect("Failed to create work directory");
    }
    work_dir
}

pub fn utc_timestamp() -> Result<String> {
    use time::format_description;
    use time::OffsetDateTime;
    let utc_time = OffsetDateTime::now_utc();
    let format = format_description::parse("[year][month][day]-[hour][minute][second]")?;
    let timestamp = utc_time.format(&format)?;
    Ok(timestamp)
}
