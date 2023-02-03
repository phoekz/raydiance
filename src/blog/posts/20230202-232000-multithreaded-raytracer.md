<info
    title="Multithreaded raytracer"
    link="multithreaded-raytracer"
    date="2023-02-02"
    commit="d129b2601d8d95112cbd14a0f5e0a7c35a9953da"
/>

<video width="800" height="450" autoplay loop muted playsinline>
    <source src="media/multithreaded-raytracer/title-h265.mp4" type="video/mp4" />
    <source src="media/multithreaded-raytracer/title-vp9.webm" type="video/webm" />
</video>

Raytracer is now multithreaded with [`rayon`](https://crates.io/crates/rayon).
We split the image into 16x16 pixel tiles, and use `into_par_iter()` to render
tiles in parallel. On AMD Ryzen 5950X processor, we can render the cube scene at
66 MRays/s, up from 4.6 MRays/s we had previously with our single threaded
raytracer. If we only used one sample per pixel, it would run slightly over 60
fps. Of course the image would be very noisy, but at least it would be
interactive.

To retain our previous single threaded debugging capabilities, we can set
`RAYON_NUM_THREADS=1` to force `rayon` only use one thread.

With multithreading, there is a subtle issue with our current random number
generator, because we can no longer share the same RNG across all threads
without locking. We can sidestep the whole problem, because we can initialize
the RNG with an unique seed at each pixel, like so:

$$seed = (pixel_x + pixel_y * image_{width}) * sample_{index}$$

Multiplying by $sample_{index}$ makes sure the $seed$ is different at each
sample. With this strategy we assume that the cost of creating an RNG is
negligible compared to the rest of the raytracer, which is true with `rand_pcg`.
