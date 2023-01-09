#version 460 core
#pragma shader_stage(vertex)

layout(push_constant) uniform PushBuffer
{
    mat4 transform;
    vec4 base_color;
}
push;

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;

layout(location = 0) out vec3 frag_normal;
layout(location = 1) out vec3 frag_color;

void main()
{
    gl_Position = push.transform * vec4(in_position, 1.0);
    frag_normal = in_normal;
    frag_color = push.base_color.xyz;
}