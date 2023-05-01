#version 460 core
#include "common.glsl"

// Inputs.
layout(location = 0) in vec2 in_texcoord;
layout(location = 1) in vec3 in_normal;
layout(location = 2) perprimitiveEXT flat in uint in_base_color;

// Outputs.
layout(location = 0) out vec4 out_color;

void main() {
    // Resolve base color.
    vec3 base_color;
    if (texture_flags[in_base_color] == 1) {
        base_color = dynamic_textures[in_base_color].rgb;
    } else {
        base_color = sample_texture(in_base_color, 0, in_texcoord).rgb;
    }

    // Simple lighting.
    const vec3 light_dir = normalize(vec3(1.0, 3.0, 1.0));
    const float light = 0.5 + 0.5 * max(0.0, dot(in_normal, light_dir));

    // Output.
    out_color = vec4(light * base_color, 1.0);
}