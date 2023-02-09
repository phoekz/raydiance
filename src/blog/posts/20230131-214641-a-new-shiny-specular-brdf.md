<info
    title="A new shiny specular BRDF"
    link="a-new-shiny-specular-brdf"
    date="2023-01-31"
    commit="bf578f68f0c0906a9cb50548eb3e830cd69222d6"
/>

![](media/a-new-shiny-specular-brdf/title.png)

# A very brief overview of microfacet models

A popular way to model physically based reflective surfaces is to imagine that
such surfaces are built of small perfect mirrors called microfacets.
Conceptually, each facet has a random height and orientation. The randomness of
these properties determines the roughness of the microsurface. These microfacets
are so tiny that they can be modeled with functions, as opposed to being modeled
with geometry or with normal maps, for example.

Let's define some common terms first:

$$
\begin{aligned}
    \omega_i &= \text{incoming direction} \\
    \omega_o &= \text{outgoing direction} \\
    \omega_m &= \text{microsurface normal} \\
    \omega_g &= \text{geometric normal} \\
\end{aligned}
$$

The popular [Cook-Torrance model][cook-torrance-paper] is defined as:

$$
f(\omega_i, \omega_o) = \frac{D(\omega_m) F(\omega_i, \omega_m) G(\omega_i,
\omega_o, \omega_m)}{4 |\omega_i \cdot \omega_g| |\omega_o \cdot \omega_g|}
$$

$D(\omega_m)$ is the _normal distribution function_, or NDF. It describes how
microfacet varies related to the microsurface normal $\omega_m$. Disney model
uses the widely popular [GGX distribution][ggx-paper], so that is what we are
going to use as well.

$F(\omega_i, \omega_m)$ describes how much light is reflected from a microfacet.
We use the same Schlick's approximation as we did with Disney diffuse BRDF.

$G(\omega_i, \omega_o, \omega_m)$ is the masking-shadowing function. It
describes the ratio of masked or shadowed microfacets when viewed from a pair of
incoming and outgoing directions. We implemented Smith's height-correlated
masking-shadowing function from this great paper by Eric Heitz called
[Understanding the Masking-Shadowing Function in Microfacet-Based
BRDFs][g-function-paper].

# Microfacet models in practice

<article-image>
    <article-caption-image>
        <img src="media/a-new-shiny-specular-brdf/bug.png"></img>
        What happens if we use a wrong coordinate system
    </article-caption-image>
</article-image>

Translating math into source code has a couple of gotchas we need to be aware
of:

- Different papers have different naming conventions (incoming vs. light,
  outgoing vs. view), and different coordinate systems (z is up vs. y is up),
  which can quickly get confusing if we are not being consistent.
- Floating point inaccuracies can make various terms go to $\infty$ or become a
  $\text{Not a Number}$. For example, if the incident or outgoing rays are close
  to being perpendicular to the surface normal, the cosine of their angles
  related to the surface normal approaches zero. Then, any expression divided by
  this value results in $\infty$. The program won't crash, but the image will
  slowly become more and more corrupted with black or white pixels. We must take
  extra care to clamp such values to a small positive number to avoid dividing
  by zero.
- Sometimes the sampled vector appears below the hemisphere. In these cases, we
  discard the whole sample because those samples have zero reflectance.

We also use the trick from `pbrt` where they perform all BRDF calculations in a
local space, where the surface normal $\omega_g=(0,1,0)$. In this local space,
many computations simplify a lot. For example, computing the dot product between
a vector against the surface normal is simply the y-component of the vector. We
can use the same orthonormal basis from the previous posts to go from world to
local space, and once we are done with all BRDF math, we can transform the
results back to world space.

# Integrating microfacets with Disney diffuse BRDF

![](media/a-new-shiny-specular-brdf/metallic-lerp.apng)

The new specular BRDF introduced three new parameters to our material:

- $metallic$ is a linear blend between $0=dielectric$ and $1=metallic$. The
  "specular color" is derived from the base color.
- $specular$ replaces the explicit index of refraction. It is currently fixed to
  $0.5$ because we don't have a way to get it from GLB yet.
- $anisotropic$ defines the degree of anisotropy. Controls the aspect ratio of
  the specular highlight. It's currently disabled because our model does not
  have tangents.

The Disney paper states that their model allows their artists to blend between
any two combinations of parameters and have good results. In the example we
interpolate metallic from $0$ to $0.5$ to $1$.

We now have an interesting problem: choosing which BRDF to sample from. The
Disney paper doesn't describe a method for it, so in our implementation, we draw
a new random variable that selects between diffuse and specular BRDF based on
the metallic parameter. For example, if the metallic value is $0.5$, both
diffuse and specular BRDFs are equally likely to be chosen.

# Animated BRDF visualizations

<article-image-pair>
    <article-caption-image>
        <img src="media/a-new-shiny-specular-brdf/microfacet-reflection-r-scalar-sobol-hemisphere.png"/>
    </article-caption-image>
    <article-caption-image>
        <img src="media/a-new-shiny-specular-brdf/microfacet-reflection-r-incoming-sobol-hemisphere.png"/>
    </article-caption-image>
    <article-caption-image>
        <img src="media/a-new-shiny-specular-brdf/microfacet-reflection-r-scalar-sobol-angle.png"/>
        Fixed incoming direction<br/>
        Roughness interpolates between $0$ and $1$
    </article-caption-image>
    <article-caption-image>
        <img src="media/a-new-shiny-specular-brdf/microfacet-reflection-r-incoming-sobol-angle.png"/>
        Incoming direction interpolates along x-axis<br/>
        Fixed roughness to $0.25$
    </article-caption-image>
</article-image-pair>

We dramatically improved the capabilities of the sample placement visualizer
from the previous post. The visualizations are now animated and can render
different text for each frame, and reflectance is directly visualized separately
from probability density functions.

The animations are encoded in [APNG][apng-wiki] format. We chose APNG because:

- GIF is too low quality due to limited 256-color palette limitation
- WebP's crate takes very long to build and has slightly worse support than APNG
- Traditional video formats are not as convenient for short looping animations

We used these crates to create the animations:

- [`apng`][apng-crate] for encoding APNG's from [`image`][image-crate] buffers.
- [`png`][png-crate] so we can properly call into [`apng`][apng-crate].
- [`easer`][easer-crate] for [easing functions][easings] We use `easeInOutCubic` for interesting movement.
- [`imageproc`][imageproc-crate] for drawing text on [`image`][image-crate] buffers.
- [`rusttype`][rusttype-crate] for loading TTF fonts for [`imageproc`][imageproc-crate].
- [`rayon`][rayon-crate] for simple data-parallelism to speed up animation renders.

# A simple interactive material editor

<video width="800" height="450" autoplay loop muted playsinline>
    <source src="media/a-new-shiny-specular-brdf/material-editor-h265.mp4" type="video/mp4" />
    <source src="media/a-new-shiny-specular-brdf/material-editor-vp9.webm" type="video/webm" />
</video>

Having to recompile the program or exporting a new scene from Blender every time
we needed to change the roughness or metallic value quickly became a significant
bottleneck. Since our raytracing is already progressive, we can quickly
implement simple material edits and have the raytracer re-render the image at
each change.

We will rewrite this utility after a more extensive user interface overhaul.

# Visualizing normals

<article-image-pair>
    <article-caption-image>
        <img src="media/a-new-shiny-specular-brdf/normal-raytraced.png" width="100%"/>
        Raytraced
    </article-caption-image>
    <article-caption-image>
        <img src="media/a-new-shiny-specular-brdf/normal-rasterized.png" width="100%"/>
        Rasterized
    </article-caption-image>
</article-image-pair>

While hunting for bugs in our specular BRDFs, we added a simple way to visualize
shading normals in raytraced and rasterized scenes. We will add visualizations
for texture coordinates and tangents in the future.

[cook-torrance-paper]: https://inst.eecs.berkeley.edu//~cs283/sp13/lectures/cookpaper.pdf
[ggx-paper]: https://www.cs.cornell.edu/~srm/publications/EGSR07-btdf.pdf
[g-function-paper]: https://jcgt.org/published/0003/02/03/paper.pdf
[apng-wiki]: https://en.wikipedia.org/wiki/APNG
[apng-crate]: https://crates.io/crates/apng
[image-crate]: https://crates.io/crates/image
[png-crate]: https://crates.io/crates/png
[easer-crate]: https://crates.io/crates/easer
[easings]: https://easings.net/
[imageproc-crate]: https://crates.io/crates/imageproc
[rusttype-crate]: https://crates.io/crates/rusttype
[rayon-crate]: https://crates.io/crates/rayon
