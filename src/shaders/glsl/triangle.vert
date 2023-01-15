#version 460 core
#pragma shader_stage(vertex)

layout(push_constant) uniform PushBuffer
{
    mat4 transform;
}
push;

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec2 in_tex_coord;
layout(location = 2) in vec3 in_normal;

layout(location = 0) out vec2 frag_tex_coord;
layout(location = 1) out vec3 frag_normal;

void main()
{
    gl_Position = push.transform * vec4(in_position, 1.0);
    frag_tex_coord = in_tex_coord;
    frag_normal = in_normal;
}