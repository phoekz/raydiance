<info
    title="Adding multisampled anti-aliasing (MSAA)"
    link="adding-multisampled-antialiasing-msaa"
    date="2023-01-09"
    commit="ca2a23caaa5e9d9b321a50389af492fbc708b560"
/>

![](media/adding-multisampled-antialiasing-msaa/title.png)

This was pretty easy. Similarly to depth buffer, we create a new color buffer
which will be multisampled. The depth buffer is also updated to support
multisampling. Then we update all the `resolve*` fields in
[`VkRenderingAttachmentInfo`](https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkRenderingAttachmentInfo.html),
and finally we add multisampling state to our pipeline. No more jagged edges.
