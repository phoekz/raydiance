use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Result};

macro_rules! build_print {
    ($($tokens: tt)*) => {
        println!("cargo:warning={}", format!($($tokens)*))
    }
}

fn main() -> Result<()> {
    // Setup paths.
    let vulkan_sdk = env!("VULKAN_SDK");
    build_print!("Vulkan SDK: {vulkan_sdk}");
    let glslc = PathBuf::from(vulkan_sdk).join("Bin\\glslc.exe");
    if !glslc.exists() {
        bail!("Could not find glslc.exe from {}", glslc.display());
    }
    build_print!("Shader compiler: {}", glslc.display());
    let glsl_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src\\shaders\\glsl");
    if !glsl_dir.exists() {
        bail!("Could not find GLSL directory from {}", glsl_dir.display());
    }
    build_print!("GLSL directory: {}", glsl_dir.display());
    let spv_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src\\shaders\\spv");
    if !spv_dir.exists() {
        bail!("Could not find SPV directory from {}", spv_dir.display());
    }
    build_print!("SPV directory: {}", spv_dir.display());

    // Only detect changes under glsl directory.
    println!(
        "cargo:rerun-if-changed={}",
        glsl_dir.to_string_lossy().replace('\\', "/")
    );

    // Build shaders.
    let shaders = ["triangle.vert", "triangle.frag"];
    for shader in shaders {
        glsl_to_spv(&glslc, &glsl_dir, &spv_dir, shader)?;
    }

    Ok(())
}

fn glsl_to_spv(glslc: &Path, glsl_dir: &Path, spv_dir: &Path, shader: &str) -> Result<()> {
    let glsl = glsl_dir.join(shader);
    let spv = spv_dir.join(shader);
    if !glsl.exists() {
        bail!("Could not find {} shader from {}", shader, glsl.display());
    }
    let output = Command::new(glslc)
        .arg(glsl)
        .arg("--target-env=vulkan1.3")
        .arg("-O")
        .arg("-o")
        .arg(spv)
        .output()?;
    if output.status.success() {
        build_print!("Built {shader}");
    } else {
        build_print!("Failed {shader}: {output:?}");
        panic!();
    }
    Ok(())
}
