<info
    title="Cosine-weighted hemisphere sampling"
    link="cosine-weighted-hemisphere-sampling"
    date="2023-01-13"
    commit="2e0e31997c20714ccc6a3735cf3a5c0a899f9ab9"
/>

![](media/cosine-weighted-hemisphere-sampling/title.apng)

To get a cleaner picture, we could increase the number of samples, but that
would increase render times, which forces us to find ways to make the renderer
run faster. Alternatively, we could be smarter at using our limited number of
samples. This way of reducing noise in Monte Carlo simulations is called
[importance sampling](https://en.wikipedia.org/wiki/Importance_sampling). For
our simple diffuse cube scene, one of the most impactful techniques is
cosine-weighted hemisphere sampling. Since the rendering equation has a cosine
term, it makes sense to sample from a distribution that is similar to that. Our
implementation is based on
[`pbrt`](https://www.pbr-book.org/3ed-2018/Monte_Carlo_Integration/2D_Sampling_with_Multidimensional_Transformations#Cosine-WeightedHemisphereSampling).

<article-image-pair>
    <article-caption-image>
        <img src="media/cosine-weighted-hemisphere-sampling/uniform.apng"/>
        Uniform sampling
    </article-caption-image>
    <article-caption-image>
        <img src="media/cosine-weighted-hemisphere-sampling/cosine.apng"/>
        Cosine-weighted sampling
    </article-caption-image>
</article-image-pair>

Here is the comparison between uniform sampling. It is clear that with identical
sample counts, cosine-weighted sampling results in much cleaner picture than
uniform sampling. And it does it at pretty much equivalent time.
