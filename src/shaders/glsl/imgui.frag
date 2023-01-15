#version 460 core
#pragma shader_stage(fragment)

layout(binding = 0) uniform sampler2D image_sampler;

layout(location = 0) in vec2 frag_uv;
layout(location = 1) in vec4 frag_color;

layout(location = 0) out vec4 out_color;

void main()
{
    vec4 linear_color = frag_color * texture(image_sampler, frag_uv);
    out_color = linear_color;
}