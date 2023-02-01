<info
    title="More triangles, cameras, light, and depth"
    link="more-triangles-cameras-light-and-depth"
    date="2023-01-09"
    commit="cb1bcc1975e3860b7208cffb4286fec3e91cc5d2"
/>

![spinning cube](images/20230109-181800.webp)

A lot has happened since our single hardcoded triangle. We can now render
shaded, depth tested, transformed, indexed triangle lists, with perspective
projection.

# Loading and rendering GLTF scenes

![](images/20230109-182433.png)

We created a simple "cube on a plane" scene in Blender. Each object has a
"Principled BSDF" material attached to it. This material is [well
supported](https://docs.blender.org/manual/en/latest/addons/import_export/scene_gltf2.html#extensions)
by Blender's GLTF exporter, which is what we will use for our application. GLTF
supports text formats, but we will export the scene in binary (`.glb`) for
efficiency.

To load the `.glb` file, we use [`gltf`](https://crates.io/crates/gltf) crate.
Immediately after loading, we pick out the interesting fields (cameras, meshes,
materials) and convert them into our [internal data
format](https://github.com/phoekz/raydiance/blob/cb1bcc1975e3860b7208cffb4286fec3e91cc5d2/src/assets.rs#L3-L35).
This internal format is designed to be easy to upload to the GPU. We also do
aggressive validation in order to catch any properties that we don't support
yet, such as textures, meshes that do not have normals, and so on. Our internal
formats represent matrices and vectors with types from
[`nalgebra`](https://crates.io/crates/nalgebra) crate. To turn our internal
formats into byte slices [`bytemuck`](https://crates.io/crates/bytemuck) crate.

Before we can render, we need to upload geometry data to the GPU. For now, we
assume the number of meshes is much less than 4096 (on most Windows hosts the
[`maxMemoryAllocationCount`](https://vulkan.gpuinfo.org/displaydevicelimit.php?platform=windows&name=maxMemoryAllocationCount)
is 4096). This allows us to cheat and allocate buffers for each mesh. The better
way to handle allocations is make a few large allocations and sub-allocate
within those, which we can do ourselves, or use a library like
[`VulkanMemoryAllocator`](https://github.com/GPUOpen-LibrariesAndSDKs/VulkanMemoryAllocator).
We will come back to memory allocators in the future.

To render, we will have to work out the perspective projection, the view
transform and object transforms from GLTF. We also add rotation transform to
animate the cube. We pre-multiply all transforms and upload the final matrix to
the vertex shader using [push
constants](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#descriptorsets-push-constants).
The base color of the mesh is also packed into a push constant. Push constants
are great for small data, because we can avoid:

1. Descriptor set layouts, descriptor pools, descriptor sets
2. Uniform buffers, which would have to be double buffered to avoid stalls
3. Synchronizing updates to uniform buffers

As a side, while looking into push constants, we learned about
[`VK_KHR_push_descriptor`](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#VK_KHR_push_descriptor).
This extension sounds like it could further simplify working with Vulkan, which
is really exciting. We will come back to it in the future once we get into
texture mapping.

# Depth testing with `VK_KHR_dynamic_rendering`

![](images/20230109-190941.png)

Depth testing requires a depth texture, which we create at startup, and
re-create when the window changes size. To enable depth testing with
`VK_KHR_dynamic_rendering`, we had to extend our graphics pipeline with a new
structure called
[`VkPipelineRenderingCreateInfo`](https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#VkPipelineRenderingCreateInfo),
and also add color blend state which was previously left out. One additional
pipeline barrier was required to transition the depth texture for rendering.
