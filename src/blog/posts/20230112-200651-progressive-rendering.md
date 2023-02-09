<info
    title="Progressive rendering"
    link="progressive-rendering"
    date="2023-01-12"
    commit="f50c3b6f92eedd52e43406ee4960d3b50b5025ac"
/>

![](media/progressive-rendering/title.apng)

Previously we had to wait until the renderer completed the entire image before
displaying it on the screen. In this commit, we redesigned the path tracing loop
to render progressively and submit intermediate frames as soon as they are
finished. This change significantly improves interactivity.
