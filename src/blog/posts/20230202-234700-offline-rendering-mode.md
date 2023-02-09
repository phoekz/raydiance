<info
    title="Offline rendering mode"
    link="offline-rendering-mode"
    date="2023-02-02"
    commit="17e72dada89f79bdc220726be7089aedc419dc3f"
/>

<video width="800" height="450" autoplay loop muted playsinline>
    <source src="media/offline-rendering-mode/title-h265.mp4" type="video/mp4" />
    <source src="media/offline-rendering-mode/title-vp9.webm" type="video/webm" />
</video>

The raytracer can now run "offline," which means that we never start up the
Vulkan rasterizer, and the program exits after rendering finishes. This mode can
generate offline rendered animations at high sample counts. The title animation
contains 60 frames, each rendered at 256 samples per pixel.
