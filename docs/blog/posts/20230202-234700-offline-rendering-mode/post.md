{{Meta((title:"Offline rendering mode", commit:"17e72dada89f79bdc220726be7089aedc419dc3f"))}}

{{Video((h265:"title-h265.mp4", vp9:"title-vp9.webm"))}}

The raytracer can now run "offline," which means that we never start up the
Vulkan rasterizer, and the program exits after rendering finishes. This mode can
generate offline rendered animations at high sample counts. The title animation
contains 60 frames, each rendered at 256 samples per pixel.
