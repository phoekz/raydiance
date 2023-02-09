{{Meta((title:"Cosine-weighted hemisphere sampling", commit:"2e0e31997c20714ccc6a3735cf3a5c0a899f9ab9"))}}

![](title.apng)

To get a clearer picture, we could increase the number of samples, which would
increase render times, forcing us to find ways to make the renderer run faster.
Alternatively, we could be smarter at using our limited number of samples. This
way of reducing noise in Monte Carlo simulations is called [importance
sampling][importance-sampling-wiki]. One of the most impactful techniques for
our simple diffuse cube scene is cosine-weighted hemisphere sampling. Since the
rendering equation has a cosine term, it makes sense to sample from a similar
distribution. We based our implementation on [`pbrt`][importance-sampling-pbrt].

{{ImagePair((left:"uniform.apng", left_text:"Uniform sampling", right:"cosine.apng", right_text:"Cosine-weighted sampling"))}}

Here is the comparison between uniform sampling. It is clear that with identical
sample counts, cosine-weighted sampling results in a much clearer picture than
uniform sampling. And it does it at an equivalent time.

[importance-sampling-wiki]: https://en.wikipedia.org/wiki/Importance_sampling
[importance-sampling-pbrt]: https://www.pbr-book.org/3ed-2018/Monte_Carlo_Integration/2D_Sampling_with_Multidimensional_Transformations#Cosine-WeightedHemisphereSampling
