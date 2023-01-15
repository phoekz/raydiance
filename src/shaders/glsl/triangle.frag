#version 460 core
#pragma shader_stage(fragment)

layout(binding = 0) uniform sampler2D image_sampler;

layout(location = 0) in vec2 frag_tex_coord;
layout(location = 1) in vec3 frag_normal;

layout(location = 0) out vec4 out_color;

void main()
{
    vec3 base_color = texture(image_sampler, frag_tex_coord).rgb;
    vec3 light_dir = normalize(vec3(1.0, 3.0, 1.0));
    float light = 0.5 + 0.5 * max(0.0, dot(frag_normal, light_dir));
    out_color = vec4(light * base_color, 1.0);
}