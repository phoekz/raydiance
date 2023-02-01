<info
    title="Interactive CPU path tracer"
    link="interactive-cpu-path-tracer"
    date="2023-01-12"
    commit="956e4bf6a4fdb2db5ea790acac1227c0297e58ac"
/>

![](images/20230112-152800.webp)

This commit merges the CPU path tracer with the Vulkan renderer, and makes the
camera interactive. As soon as the path tracer finishes rendering, the image is
uploaded to the GPU and rendered on the window. We can also toggle between
raytraced image and rasterized image to confirm that both renderers are in sync.

To keep the Vulkan renderer running while the CPU is busy path tracing, we need
to run the path tracer on its own thread. To communicate across threads
boundaries, we use Rust standard library
[`std::sync::mpsc:channel`](https://doc.rust-lang.org/std/sync/mpsc/index.html).
The main thread sends camera transforms to the path tracer, and the path tracer
sends the rendered images back to the main thread. The path tracer thread blocks
on the channel in prevent busy looping.

For displaying path traced images on the window, we set up a both the uploader
and the rendering pipeline for the image. Uploading image data is not very
exciting, refer to this page from [Vulkan
Tutorial](https://vulkan-tutorial.com/Texture_mapping/Images).

However, for rendering we used two tricks:

- To render a fullscreen textured quad, you don't actually need to create vertex
buffers, set up vertex inputs, and so on. With this
[trick](https://www.saschawillems.de/blog/2016/08/13/vulkan-tutorial-on-rendering-a-fullscreen-quad-without-buffers/),
you can use `gl_VertexIndex` intrinsic in the vertex shader to build a huge
triangle and then calculate the UVs within it. This saves a lot of boilerplate.
- In Vulkan if you want to sample a texture in your fragment shader, you need to
create descriptor pools, descriptor set layouts, allocate descriptor sets, make
sure pipeline layouts are correct, bind the descriptor sets, and so on. With
[`VK_KHR_push_descriptor`](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#VK_KHR_push_descriptor)
extension, it is possible to simplify this process significantly. Enabling it
allows you to push the descriptor right before issuing the draw call, saving a
lot of boilerplate. We still have to create one descriptor set layout for the
pipeline layout object, but that is not too bad compared to what we had to do
before, just to bind one texture to a shader.

As a side, `vulkan.rs` is reaching 2000 LOC, which is getting pretty challenging
to work with. We will have to break it down soon.

The path tracing performance is not great because we are still using only one
thread. It is also why the image is noisier than the previous post, since we had
to lower the sample count to get barely interactive frame rates. We will address
the noise and the performance in upcoming commits.
