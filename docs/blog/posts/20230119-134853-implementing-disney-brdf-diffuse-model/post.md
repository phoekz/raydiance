{{Meta((title:"Implementing Disney BRDF - Diffuse model", commit:"c43f282e24a6eecb54fa3361a6e5e192453d0d8d"))}}

![](title.apng)

[Disney principled BRDF][disney-paper] is a popular physically based reflectance
model developed by Brent Burley et al. at Disney. It is adopted by
[Blender][blender], [Unreal Engine][unreal], [Frostbite][frostbite], and many
other productions. The team at Disney analyzed the [MERL BRDF Database][merl]
and fit their model based on MERL's empirical measurements. Their goal was to
create an artist-friendly model with as few parameters as possible. These are
the design principles from the course notes:

1. Intuitive rather than physical parameters should be used.
2. There should be as few parameters as possible.
3. Parameters should be zero to one over their plausible range.
4. Parameters should be allowed to be pushed beyond their plausible range where it makes sense.
5. All combinations of parameters should be as robust and plausible as possible.

The full Disney model combines multiple scattering models, some of which we need
to become more experienced with. To avoid getting overwhelmed, we will study and
implement one model at a time, starting with the diffuse model $f_d$, which is
defined as:

$$
\begin{aligned}
    f_d &= \frac{baseColor}{\pi} f_{di} f_{do} \\
    f_{di} &= 1 + (F_{D90} - 1)(1 - \cos\theta_i)^5 \\
    f_{do} &= 1 + (F_{D90} - 1)(1 - \cos\theta_o)^5 \\
    F_{D90} &= 0.5 + 2 roughness \cos^2\theta_d \\
    \theta_i &= |\omega_i \cdot \omega_g| \\
    \theta_o &= |\omega_o \cdot \omega_g| \\
    \theta_d &= |\omega_i \cdot \omega_m| \\
    \omega_i &= \text{incoming direction} \\
    \omega_o &= \text{outgoing direction} \\
    \omega_m &= \text{microsurface normal} \\
    \omega_g &= \text{geometric normal} \\
\end{aligned}
$$

Disney's diffuse model is a novel empirical model which attempts to solve the
over-darkening that comes from the Lambertian diffuse model. This darkening
happens at grazing angles, i.e., the angle between the incoming and outgoing
light is close to $0$. Disney models this by adding a [Fresnel
factor][fresnel-wiki], which they approximated with [Schlick's
approximation][schlick-wiki].

The difference in the comparison above can be subtle. The main difference is
that the cube's edges are slightly brighter compared to the Lambert model, and
the right side of the cube also appears brighter.

Our implementation ignores the "sheen" term and the subsurface scattering
approximation. We will come back to these terms later. Also, any roughness value
below $1$ looks incorrect because our current implementation has no specular
terms.

References:

- [SIGGRAPH 2012 - Physically Based Shading at Disney - Course Notes][disney-notes]
- [Joe Schutte - Rendering the Moana Island Scene Part 1: Implementing the Disney BSDF][joe-schutte]
- [Shih-Chin - Implementing Disney Principled BRDF in Arnold][shih-chin]
- [`pbrt-v3 - disney.cpp`][disney-pbrt]
- [Disney BRDF Explorer][disney-brdf]

[disney-paper]: https://blog.selfshadow.com/publications/s2012-shading-course/burley/s2012_pbs_disney_brdf_notes_v3.pdf
[disney-notes]: https://blog.selfshadow.com/publications/s2012-shading-course/burley/s2012_pbs_disney_brdf_notes_v3.pdf
[joe-schutte]: https://schuttejoe.github.io/post/disneybsdf/
[shih-chin]: http://shihchinw.github.io/2015/07/implementing-disney-principled-brdf-in-arnold.html
[disney-pbrt]: https://github.com/mmp/pbrt-v3/blob/master/src/materials/disney.cpp
[disney-brdf]: https://github.com/wdas/brdf
[blender]: https://docs.blender.org/manual/en/latest/render/shader_nodes/shader/principled.html
[unreal]: https://cdn2.unrealengine.com/Resources/files/2013SiggraphPresentationsNotes-26915738.pdf
[frostbite]: https://seblagarde.files.wordpress.com/2015/07/course_notes_moving_frostbite_to_pbr_v32.pdf
[merl]: https://www.merl.com/brdf/
[fresnel-wiki]: https://en.wikipedia.org/wiki/Fresnel_equations
[schlick-wiki]: https://en.wikipedia.org/wiki/Schlick%27s_approximation
