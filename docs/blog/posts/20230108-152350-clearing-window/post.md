{{Meta((title:"Clearing window with VK_KHR_dynamic_rendering", commit:"0f6d7f1bf1b22d1fff43e87080c854eadb3e459d"))}}

![resizable color window](title.apng)

After around 1000 LOC, we have a barebones Vulkan application which:

1. Load Vulkan with [`ash`][ash-crate] crate.
2. Creates Vulkan instance with `VK_LAYER_KHRONOS_validation` and debug
   utilities.
3. Creates window surface with [`ash-window`][ash-window-crate] and
   [`raw-window-handle`][raw-window-handle-crate] crates.
4. Creates a logical device and queues.
5. Creates command pool and buffers.
6. Creates the swapchain.
7. Creates semaphores and fences for host-to-host and host-to-device
   synchronization.
8. Clears the screen with a different color for every frame.

We also handle tricky situations, such as the user resizing the window and
minimizing the window.

We don't have to create render passes or framebuffers, thanks to the
`VK_KHR_dynamic_rendering` extension. However, we have to specify some render
pass parameters when we record command buffers, but reducing the number of API
abstractions simplifies the implementation. We used this
[example][dynamic-rendering] by Sascha Willems as a reference.

We wrote everything under the `main()` with minimal abstractions and liberal use
of the `unsafe` keyword. We will do a [semantic compression][casey] pass later
once we learn more about how to structure the program.

Next we will continue with more Vulkan code to get a triangle on the screen.

[ash-crate]: https://crates.io/crates/ash
[ash-window-crate]: https://crates.io/crates/ash-window
[raw-window-handle-crate]: https://crates.io/crates/raw-window-handle
[dynamic-rendering]: https://github.com/SaschaWillems/Vulkan/blob/313ac10de4a765997ddf5202c599e4a0ca32c8ca/examples/dynamicrendering/dynamicrendering.cpp
[casey]: https://caseymuratori.com/blog_0015
