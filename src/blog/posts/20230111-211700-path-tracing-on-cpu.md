<info
    title="Path tracing on CPU"
    link="path-tracing-on-cpu"
    date="2023-01-11"
    commit="4ade2d5b2acc3da8fabb5d275b9152171ed01ea9"
/>

![](media/path-tracing-on-cpu/title.png)

Finally, we are getting into the main feature of `raydiance`: rendering pretty
images using path tracing. We start with a pure CPU implementation. The plan is
to develop and maintain the CPU version as the reference implementation for the
future GPU version, mainly because it is much easier to work with, especially
when debugging shaders. The Vulkan renderer we've built so far serves as the
visual interface for `raydiance`, and later, we will use Vulkan's ray tracing
extensions to create the GPU version.

We use the following components:

- Ray vs triangle intersection: [Watertight Ray/Triangle Intersection][watertight-paper]
- Orthonormal basis: [Building an Orthonormal Basis, Revisited][onb-paper]
- Uniformly distributed random numbers: [`rand`][rand-crate] and [`rand_pcg`][rand-pcg-grate] crates
- Uniform hemisphere sampling: [`pbrt`][uniform-pbrt]
- Acceleration structure (bounding volume hierarchy): [`pbrt`][bvh-pbrt]

We put this together into a loop, where we bounce rays until they hit the sky or
have bounced too many times. Each pixel in the image does this several times,
averages all the samples, and writes out the final color to the image buffer.

For materials, we start with the simplest one: Lambertian material, which
scatters incoming light equally in all directions. However, a subtle detail in
Lambertian BRDF is that you have to divide the base color with $\pi$. Here's the
explanation from [`pbrt`][lambertian-pbrt].

For lights, we assume that every ray that bounces off the scene will hit “the
sky.” In that case, we return a bright white color.

For anti-aliasing, we randomly shift the subpixel position of each primary ray
and apply the box filter over the samples. With enough samples, this naturally
resolves into a nice image with no aliasing. [`pbrt`][image-pbrt]'s image
reconstruction chapter has better alternatives for the box filter, which we
might look into later.

We currently run the path tracer in a single CPU thread. This could be better,
but the rendering only takes a couple of seconds for such a tiny image and a low
sample count. We will return to this once we need to make the path tracer run at
interactive speeds.

Currently, raydiance doesn't display the path traced image anywhere. For this
post, we wrote the image out directly into the disk. We will fix this soon.

[watertight-paper]: https://jcgt.org/published/0002/01/05/
[onb-paper]: https://jcgt.org/published/0006/01/01/
[rand-crate]: https://crates.io/crates/rand
[rand-pcg-grate]: https://crates.io/crates/rand_pcg
[uniform-pbrt]: https://www.pbr-book.org/3ed-2018/Monte_Carlo_Integration/2D_Sampling_with_Multidimensional_Transformations#UniformlySamplingaHemisphere
[bvh-pbrt]: https://www.pbr-book.org/3ed-2018/Primitives_and_Intersection_Acceleration/Bounding_Volume_Hierarchies
[lambertian-pbrt]: https://www.pbr-book.org/3ed-2018/Reflection_Models/Lambertian_Reflection
[image-pbrt]: https://www.pbr-book.org/3ed-2018/Sampling_and_Reconstruction/Image_Reconstruction
