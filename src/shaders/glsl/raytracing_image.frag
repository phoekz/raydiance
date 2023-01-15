#version 460 core
#pragma shader_stage(fragment)

layout(binding = 0) uniform sampler2D image_sampler;

layout(location = 0) in vec2 frag_tex_coord;

layout(location = 0) out vec4 out_color;

void main()
{
    vec3 rgb = texture(image_sampler, frag_tex_coord).rgb;
    out_color = vec4(rgb, 1.0);
}