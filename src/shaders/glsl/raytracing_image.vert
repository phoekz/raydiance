#version 460 core
#pragma shader_stage(vertex)

layout(location = 0) out vec2 frag_uv;

void main()
{
    frag_uv = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    gl_Position = vec4(2.0 * frag_uv - 1.0, 0.0, 1.0);
    gl_Position.y = -gl_Position.y;
}