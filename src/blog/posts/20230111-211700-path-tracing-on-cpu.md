<info
    title="Path tracing on CPU"
    link="path-tracing-on-cpu"
    date="2023-01-11"
    commit="4ade2d5b2acc3da8fabb5d275b9152171ed01ea9"
/>

![](images/20230111-231544.png)

Finally, we are getting into the main feature of `raydiance`: rendering pretty
images using ray tracing. We start with a pure CPU implementation. The plan is
to develop and maintain the CPU version as the reference implementation for the
future GPU version, mainly because it is much easier to work with compared to
debugging shaders. The Vulkan renderer we've built so far serves as the visual
interface for `raydiance`, and later, we will use Vulkan's ray tracing
extensions to build the GPU version.

Our implementation use the following components:

- Ray vs triangle intersection: [Watertight Ray/Triangle Intersection](https://jcgt.org/published/0002/01/05/)
- Orthonormal basis: [Building an Orthonormal Basis, Revisited](https://jcgt.org/published/0006/01/01/)
- Uniformly distributed random numbers: [`rand`](https://crates.io/crates/rand) and [`rand_pcg`](https://crates.io/crates/rand_pcg) crates
- Uniform hemisphere sampling: [`pbrt`](https://www.pbr-book.org/3ed-2018/Monte_Carlo_Integration/2D_Sampling_with_Multidimensional_Transformations#UniformlySamplingaHemisphere)
- Acceleration structure (bounding volume hierarchy): [`pbrt`](https://www.pbr-book.org/3ed-2018/Primitives_and_Intersection_Acceleration/Bounding_Volume_Hierarchies)

We put this together into a path tracing loop, where we bounce rays until they
hit the sky or they have bounced too many times. Each pixel in the image does
this a number of times, averages all the samples and writes out the final color
to the image

For materials, we start with the simplest one: Lambertian material, which
scatters incoming light equally in all directions. However, there is a subtle
detail in Lambertian BRDF, which is that you have to divide the base color with
π. Here's the explanation from
[`pbrt`](https://www.pbr-book.org/3ed-2018/Reflection_Models/Lambertian_Reflection).

For lights, we assume that every ray that bounces off the scene will hit “the
sky”. In that case, we just return some bright white color.

For anti-aliasing, we randomly shift the subpixel position of each primary ray
and apply the box filter over the samples. With enough samples, this naturally
resolves into a nice image with no aliasing.
[`pbrt`](https://www.pbr-book.org/3ed-2018/Sampling_and_Reconstruction/Image_Reconstruction)'s
image reconstruction chapter has better alternatives for box filter, which we
might look into later.

For performance, we currently run the path tracer in a single CPU thread.
Obviously this is not ideal, but for such a tiny image and low sample count, the
rendering only takes a couple of seconds. We will come back to this once we need
to make the path tracer run at interactive speeds.

Currently raydiance doesn't display the path traced image anywhere, for this
post we wrote the image out to the disk. We will fix this soon.
