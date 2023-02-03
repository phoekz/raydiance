<info
    title="Visualizing sample placement"
    link="visualizing-sample-placement"
    date="2023-01-19"
    commit="f5b806749234a4086d84388d7264c0f2fd43122a"
/>

![](media/visualizing-sample-placement/title-top.png)
![](media/visualizing-sample-placement/title-side.png)

Importance sampling Disney specular models is more challenging than our current
diffuse models. To improve our chances, we created a small tool which visualizes
where the sample are placed around the hemisphere.

To make sure the tool works, we used our existing uniform and cosine-weighted
hemisphere samplers. We also added two additional sample sequences for
comparison:

1. `grid` sequence, which uniformly samples the unit square.
2. `sobol` sequence, which is provided by [`sobol_burley`](https://crates.io/crates/sobol_burley) crate. The implementation is based on [Practical Hash-based Owen Scrambling](https://www.jcgt.org/published/0009/04/01/).

In terms of coordinate spaces, the `top` plots view the hemisphere from above in
cartesian space. The `side` plots in $x=\phi=[0,2\pi]$ and $y=\theta=[0,\frac{\pi}{2}]$
hemispherical space.

For both sets of plots, the background brightness correspond to the magnitude of
$\cos\theta$.

Looking at the plots, we can intuitively say that `cosine` performs better than
`uniform` sampling, because it places samples closer to the bright spots.
Similarly, `sobol` performs better than `random` and `grid`.

Note that the new sequences are not currently available for rendering. We will
revisit low-discrepancy sequences later.
