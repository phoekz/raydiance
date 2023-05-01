#version 460 core
#include "common.glsl"

layout(local_size_x = 1) in;
layout(local_size_y = 1) in;
layout(local_size_z = 1) in;

uint mesh_group_count(uint triangle_count) {
    return (triangle_count + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
}

void main() {
    Mesh mesh = meshes[pc.mesh_index];
    uint mesh_group_x = mesh_group_count(mesh.triangle_count);
    EmitMeshTasksEXT(mesh_group_x, 1, 1);
}