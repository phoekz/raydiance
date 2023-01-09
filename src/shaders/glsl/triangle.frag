#version 460 core
#pragma shader_stage(fragment)

layout(location = 0) in vec3 frag_normal;
layout(location = 1) in vec3 frag_color;

layout(location = 0) out vec4 out_color;

void main()
{
    vec3 light_dir = normalize(vec3(1.0, 3.0, 1.0));
    float light = 0.5 + 0.5 * max(0.0, dot(frag_normal, light_dir));
    out_color = vec4(light * frag_color, 1.0);
}