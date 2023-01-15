#version 460 core
#pragma shader_stage(vertex)

layout(push_constant) uniform PushBuffer
{
    vec2 scale;
    vec2 translation;
}
push;

layout(location = 0) in vec2 in_position;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;

layout(location = 0) out vec2 frag_uv;
layout(location = 1) out vec4 frag_color;

vec4 srgb_to_linear(vec4 srgb_color)
{
    vec3 srgb = srgb_color.rgb;
    vec3 selector = ceil(srgb - 0.04045);
    vec3 less_than_branch = srgb / 12.92;
    vec3 greater_than_branch = pow((srgb + 0.055) / 1.055, vec3(2.4));
    return vec4(
        mix(less_than_branch, greater_than_branch, selector),
        srgb_color.a);
}

void main()
{
    gl_Position = vec4(in_position * push.scale + push.translation, 0.0, 1.0);
    frag_uv = in_uv;
    frag_color = srgb_to_linear(in_color);
}