<info
    title="Interactive CPU path tracer"
    link="interactive-cpu-path-tracer"
    date="2023-01-12"
    commit="956e4bf6a4fdb2db5ea790acac1227c0297e58ac"
/>

![](media/interactive-cpu-path-tracer/title.apng)

This commit merges the CPU path tracer with the Vulkan renderer and makes the
camera interactive. As soon as the path tracer finishes rendering, the image is
uploaded to the GPU and rendered on the window. We can also toggle between
raytraced and rasterized images to confirm that both renderers are in sync.

To keep the Vulkan renderer running while the CPU is busy path tracing, we need
to run the path tracer on a separate thread. To communicate across thread
boundaries, we use Rust standard library [`std::sync::mpsc:channel`][mpsc-rust].
The main thread sends camera transforms to the path tracer, and the path tracer
sends the rendered images back to the main thread. The path tracer thread blocks
the channel to prevent busy looping.

For displaying path traced images on the window, we set up both the uploader and
the rendering pipeline for the image.

We used two tricks to render the image:

- To render a fullscreen textured quad, you don't need to create vertex buffers,
  set up vertex inputs, etc. With this [trick][quad-tutorial], you can use
  `gl_VertexIndex` intrinsic in the vertex shader to build a huge triangle and
  then calculate its UVs. This technique saves a lot of boilerplate code.
- In Vulkan, if you want to sample a texture in your fragment shader, you need
  to create descriptor pools, and descriptor set layouts, allocate descriptor
  sets, make sure pipeline layouts are correct, bind the descriptor sets, and so
  on. With [`VK_KHR_push_descriptor`][push-desc-ext] extension, it is possible
  to simplify this process significantly. Enabling it allows you to push the
  descriptor right before issuing the draw call, saving a lot of boilerplate. We
  still have to create one descriptor set layout for the pipeline layout object,
  but that is pretty good compared to what we had to do before, only to bind one
  texture to a shader.

As a side, `vulkan.rs` is reaching 2000 LOC, which is getting challenging to
work with. We will have to break it down soon.

The path tracing performance could be better because we are still using only one
thread. It is also why the image is noisier than the previous post since we had
to lower the sample count to get barely interactive frame rates. We will address
the noise and the performance in upcoming commits.

[mpsc-rust]: https://doc.rust-lang.org/std/sync/mpsc/index.html
[vk-tutorial]: https://vulkan-tutorial.com/Texture_mapping/Images
[quad-tutorial]: https://www.saschawillems.de/blog/2016/08/13/vulkan-tutorial-on-rendering-a-fullscreen-quad-without-buffers/
[push-desc-ext]: https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#VK_KHR_push_descriptor
