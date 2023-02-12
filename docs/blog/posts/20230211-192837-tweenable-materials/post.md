{{Meta((title:"Tweenable materials, Disney model bug fix", commit:"66ca414cece5f32349c2a94b00f50b712e005596"))}}

{{Video((h265:"title-h265.mp4", vp9:"title-vp9.webm"))}}

Expanding on the skylight model post, we have also made all material properties in a scene tweenable with keyframes. The title video animates multiple material properties and sky model properties simultaneously.

In each video below, we interpolate one material property from $0$ to $1$ while keeping other parameters the same. In this release, we also exposed $specular$ and $specularTint$ parameters, previously hardcoded as $0.5$ and $0$, respectively.

{{Video((h265:"materials-h265.mp4", vp9:"materials-vp9.webm"))}}

# Disney model bug fix

We fixed how the $specular$ parameter worked in our Disney BRDF implementation. Previously, sliding the $specular$ parameter between $0$ to $1$ barely changed the material's appearance, while in the Disney paper, the amount of reflection goes to $0$ as the $specular$ goes to $0$. The problem was in mixing diffuse and specular BRDFs: we only accounted for the $metallic$ parameter. Unsurprisingly, this bug meant that the $specular$ parameter did not significantly influence the material's appearance. After the fix, our implementation looks better now that we account for $metallic$ and $specular$ parameters.

Additionally, we added the $specularTint$ parameter, which tints incident specular towards the base color. It's intended for additional artistic control.
