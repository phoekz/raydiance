<info
    title="Progressive rendering"
    link="progressive-rendering"
    date="2023-01-12"
    commit="f50c3b6f92eedd52e43406ee4960d3b50b5025ac"
/>

![](media/progressive-rendering/title.apng)

Previously we waited until the entire image was completed before displaying to
the screen. In this commit we redesigned the path tracing loop to render
progressively and submit intermediate frames as soon as they are finished. This
significantly improves interactivity.
