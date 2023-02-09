{{Meta((title:"Texture support", commit:"41a05a78d6fbba82436faece9815c4e8b3da9951"))}}

![](title.apng)

Raydiance now supports texture-mapped surfaces. We used multiple shortcuts to
get a basic implementation going:

- Only nearest-neighbor filtering is supported.
- No mipmaps.
- No anisotropic filtering.
- Only the `R8G8B8A8_UNORM` pixel format is supported.

We will revisit these shortcuts later once our scenes get more complicated.
