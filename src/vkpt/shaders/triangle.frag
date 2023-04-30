#version 460 core
#include "common.glslh"

layout(location = 0) in FragmentVertex fragment_vertex;
layout(location = 2)
    perprimitiveEXT flat in FragmentPrimitive fragment_primitive;

layout(location = 0) out vec4 fragment_color;

vec4 sample_texture(uint texture_id, vec2 texcoord) {
    return texture(
        sampler2D(textures[nonuniformEXT(texture_id)], samplers[0]),
        texcoord);
}

void main() {
    vec3 base_color;
    if (pc.base_color.a > 0.0) {
        base_color = pc.base_color.rgb;
    } else {
        vec2 texcoord = fragment_vertex.texcoord;
        uint texture_id = fragment_primitive.base_color;
        base_color = sample_texture(texture_id, texcoord).rgb;
    }

    vec3 light_dir = normalize(vec3(1.0, 3.0, 1.0));
    vec3 normal = fragment_vertex.normal;
    float light = 0.5 + 0.5 * max(0.0, dot(normal, light_dir));

    fragment_color = vec4(light * base_color, 1.0);
}