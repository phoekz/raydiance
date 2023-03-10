{{Meta((title:"The first triangle", commit:"c8f9ef2c0f3b51ddfd71f33ec086fca1531051ab"))}}

![first triangle](title.apng)

This is the simplest triangle example rendered without any device memory
allocations. The triangle is hardcoded in the vertex shader, and we index into
its attributes with vertex index.

We added a simple shader compiling step in [`build.rs`][build-rs] which builds
`.glsl` source code into `.spv` binary format using Google's [`glslc`][glslc],
which is included in [LunarG's Vulkan SDK][lunarg].

[build-rs]: https://doc.rust-lang.org/cargo/reference/build-scripts.html
[glslc]: https://github.com/google/shaderc/tree/main/glslc
[lunarg]: https://vulkan.lunarg.com/sdk/home
